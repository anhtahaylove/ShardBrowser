-- ShardX v0.2 team/fleet control plane.
--
-- Additive: nothing here touches the v1 tables. v1 rows keep working
-- unchanged, so an existing deployment migrates without a data rewrite.
--
-- Two invariants are enforced structurally rather than in application code,
-- because application code is the thing most likely to be bypassed:
--
--  1. Tenant isolation. Child rows carry tenant_id and reference their parent
--     through a COMPOSITE foreign key (tenant_id, id). A row therefore cannot
--     point at a parent in another tenant even if a query forgets its WHERE
--     clause — SQLite rejects the write.
--
--  2. The SQLite wire-integer domain, per plan 5.6. Every unsigned wire
--     integer persisted here (version, generation, fencing token, offset,
--     size, epoch, count, millisecond timestamp) is constrained to
--     0..9223372036854775807. CBOR's unsigned range is wider than SQLite's
--     signed INTEGER, so a value above i64::MAX would silently corrupt on
--     bind; the CHECK makes that a rejected write instead.

-- Mirror/cache of the external identity record ONLY. Per plan 5.5/12.4 the
-- authority is a checksummed, fsync'd file outside the SQLite backup and
-- rollback scope: restoring an old DB must not be able to roll the epoch
-- backwards. Startup reconciles this against the external record before any
-- v2 write is permitted.
CREATE TABLE v2_server_state (
    singleton              INTEGER PRIMARY KEY CHECK (singleton = 1),
    server_instance_id     BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch          INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    external_record_sha256 BLOB    NOT NULL CHECK (length(external_record_sha256) = 32),
    updated_at             TEXT    NOT NULL
);

CREATE TABLE v2_tenants (
    id                    BLOB    PRIMARY KEY CHECK (length(id) = 16),
    slug                  TEXT    NOT NULL UNIQUE,
    status                TEXT    NOT NULL CHECK (status IN ('active', 'suspended')),
    active_root_generation INTEGER NOT NULL CHECK (active_root_generation BETWEEN 0 AND 9223372036854775807),
    created_at            TEXT    NOT NULL
);

CREATE TABLE v2_accounts (
    id            BLOB    NOT NULL CHECK (length(id) = 16),
    tenant_id     BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    username      TEXT    NOT NULL,
    pw_hash       TEXT    NOT NULL,
    -- Links a v2 account to the v1 `users` row it was migrated from, so an
    -- existing session token can be resolved to a v2 account without asking
    -- the client to assert its own identity. NULL for accounts created
    -- directly in v2. Not a foreign key: v1 users may be deleted while the
    -- v2 audit trail must still resolve.
    legacy_user_id TEXT,
    token_version INTEGER NOT NULL DEFAULT 0 CHECK (token_version BETWEEN 0 AND 9223372036854775807),
    status        TEXT    NOT NULL CHECK (status IN ('active', 'disabled')),
    created_at    TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, username)
);

CREATE TABLE v2_tenant_memberships (
    tenant_id  BLOB NOT NULL,
    account_id BLOB NOT NULL,
    role       TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    status     TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, account_id),
    FOREIGN KEY (tenant_id, account_id) REFERENCES v2_accounts(tenant_id, id) ON DELETE CASCADE
);

-- Device identity. Both public keys are stored with their suite id: a key is
-- only meaningful together with the algorithm it belongs to, and pinning the
-- suite stops a key being reinterpreted under a weaker one.
CREATE TABLE v2_devices (
    id                 BLOB    NOT NULL CHECK (length(id) = 16),
    tenant_id          BLOB    NOT NULL,
    account_id         BLOB    NOT NULL,
    label_ciphertext   BLOB    NOT NULL,
    signing_key_id     BLOB    NOT NULL CHECK (length(signing_key_id) = 32),
    signing_public_key BLOB    NOT NULL CHECK (length(signing_public_key) = 32),
    signing_suite      INTEGER NOT NULL CHECK (signing_suite BETWEEN 0 AND 9223372036854775807),
    hpke_key_id        BLOB    NOT NULL CHECK (length(hpke_key_id) = 32),
    hpke_public_key    BLOB    NOT NULL CHECK (length(hpke_public_key) = 32),
    hpke_suite         INTEGER NOT NULL CHECK (hpke_suite BETWEEN 0 AND 9223372036854775807),
    status             TEXT    NOT NULL CHECK (status IN ('active', 'revoked')),
    last_seen_at       TEXT,
    created_at         TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, account_id) REFERENCES v2_accounts(tenant_id, id) ON DELETE CASCADE,
    UNIQUE (tenant_id, signing_key_id),
    UNIQUE (tenant_id, hpke_key_id)
);

CREATE TABLE v2_sessions (
    id                 BLOB NOT NULL CHECK (length(id) = 16),
    tenant_id          BLOB NOT NULL,
    account_id         BLOB NOT NULL,
    device_id          BLOB NOT NULL,
    refresh_token_hash BLOB NOT NULL CHECK (length(refresh_token_hash) = 32),
    expires_at         TEXT NOT NULL,
    revoked_at         TEXT,
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, account_id) REFERENCES v2_accounts(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, device_id) REFERENCES v2_devices(tenant_id, id) ON DELETE CASCADE
);

-- Enrolment challenges are bound to the instance and epoch that issued them,
-- so a challenge cannot be replayed across a restore.
CREATE TABLE v2_enrollment_challenges (
    id                 BLOB    NOT NULL CHECK (length(id) = 16),
    tenant_id          BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    server_instance_id BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch      INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    nonce_hash         BLOB    NOT NULL CHECK (length(nonce_hash) = 32),
    key_commitment     BLOB    NOT NULL CHECK (length(key_commitment) = 32),
    expires_at         TEXT    NOT NULL,
    consumed_at        TEXT,
    PRIMARY KEY (tenant_id, id)
);

-- Signed authorization records.
--
-- The exact signed container bytes are stored verbatim alongside the parsed
-- columns. Verification always runs against the stored bytes, never against a
-- re-encoding of the columns: the columns are an index, not the truth. Storing
-- both lets a verifier detect divergence instead of trusting the parse.
CREATE TABLE v2_device_approvals (
    tenant_id                        BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    replay_id                        BLOB    NOT NULL CHECK (length(replay_id) = 16),
    payload_domain                   TEXT    NOT NULL,
    payload_version                  INTEGER NOT NULL CHECK (payload_version BETWEEN 0 AND 9223372036854775807),
    subject_account_id               BLOB    NOT NULL CHECK (length(subject_account_id) = 16),
    subject_device_id                BLOB    NOT NULL CHECK (length(subject_device_id) = 16),
    subject_signing_key_id           BLOB    NOT NULL CHECK (length(subject_signing_key_id) = 32),
    subject_hpke_key_id              BLOB    NOT NULL CHECK (length(subject_hpke_key_id) = 32),
    approval_scope_kind              TEXT    NOT NULL,
    approval_scope_id                BLOB    NOT NULL CHECK (length(approval_scope_id) = 16),
    approved_use                     TEXT    NOT NULL,
    issued_at_ms                     INTEGER NOT NULL CHECK (issued_at_ms BETWEEN 0 AND 9223372036854775807),
    not_before_ms                    INTEGER NOT NULL CHECK (not_before_ms BETWEEN 0 AND 9223372036854775807),
    not_after_ms                     INTEGER NOT NULL CHECK (not_after_ms BETWEEN 0 AND 9223372036854775807),
    server_instance_id               BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch                    INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    canonical_payload_bytes          BLOB    NOT NULL,
    payload_sha256                   BLOB    NOT NULL CHECK (length(payload_sha256) = 32),
    signature_suite_id               INTEGER NOT NULL CHECK (signature_suite_id BETWEEN 0 AND 9223372036854775807),
    signature_version                INTEGER NOT NULL CHECK (signature_version BETWEEN 0 AND 9223372036854775807),
    signature_bytes                  BLOB    NOT NULL CHECK (length(signature_bytes) = 64),
    issuer_signing_key_id            BLOB    NOT NULL CHECK (length(issuer_signing_key_id) = 32),
    signed_container_hash            BLOB    NOT NULL CHECK (length(signed_container_hash) = 32),
    exact_signed_container_bytes     BLOB    NOT NULL,
    exact_signed_container_bytes_sha256 BLOB NOT NULL CHECK (length(exact_signed_container_bytes_sha256) = 32),
    revoked_at                       TEXT,
    created_at                       TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, payload_domain, replay_id)
);

CREATE TABLE v2_capability_grants (
    tenant_id                        BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    replay_id                        BLOB    NOT NULL CHECK (length(replay_id) = 16),
    payload_domain                   TEXT    NOT NULL,
    payload_version                  INTEGER NOT NULL CHECK (payload_version BETWEEN 0 AND 9223372036854775807),
    subject_account_id               BLOB    NOT NULL CHECK (length(subject_account_id) = 16),
    subject_device_id                BLOB    NOT NULL CHECK (length(subject_device_id) = 16),
    capability                       TEXT    NOT NULL,
    scope_kind                       TEXT    NOT NULL,
    scope_id                         BLOB    NOT NULL CHECK (length(scope_id) = 16),
    issued_at_ms                     INTEGER NOT NULL CHECK (issued_at_ms BETWEEN 0 AND 9223372036854775807),
    not_before_ms                    INTEGER NOT NULL CHECK (not_before_ms BETWEEN 0 AND 9223372036854775807),
    not_after_ms                     INTEGER NOT NULL CHECK (not_after_ms BETWEEN 0 AND 9223372036854775807),
    server_instance_id               BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch                    INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    canonical_payload_bytes          BLOB    NOT NULL,
    payload_sha256                   BLOB    NOT NULL CHECK (length(payload_sha256) = 32),
    signature_bytes                  BLOB    NOT NULL CHECK (length(signature_bytes) = 64),
    issuer_signing_key_id            BLOB    NOT NULL CHECK (length(issuer_signing_key_id) = 32),
    signed_container_hash            BLOB    NOT NULL CHECK (length(signed_container_hash) = 32),
    exact_signed_container_bytes     BLOB    NOT NULL,
    exact_signed_container_bytes_sha256 BLOB NOT NULL CHECK (length(exact_signed_container_bytes_sha256) = 32),
    revoked_at                       TEXT,
    created_at                       TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, payload_domain, replay_id)
);

-- HPKE-sealed tenant root key grants. The server stores the sealed bytes and
-- never the root key: only the recipient device's private key can open them.
CREATE TABLE v2_tenant_root_key_grants (
    tenant_id                        BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    replay_id                        BLOB    NOT NULL CHECK (length(replay_id) = 16),
    payload_domain                   TEXT    NOT NULL,
    grant_variant                    TEXT    NOT NULL CHECK (grant_variant IN ('FirstRootSelfGrant', 'CustodianIssued')),
    root_key_id                      BLOB    NOT NULL CHECK (length(root_key_id) = 32),
    root_generation                  INTEGER NOT NULL CHECK (root_generation BETWEEN 0 AND 9223372036854775807),
    grant_capability                 TEXT    NOT NULL,
    subject_account_id               BLOB    NOT NULL CHECK (length(subject_account_id) = 16),
    subject_device_id                BLOB    NOT NULL CHECK (length(subject_device_id) = 16),
    subject_signing_key_id           BLOB    NOT NULL CHECK (length(subject_signing_key_id) = 32),
    recipient_hpke_key_id            BLOB    NOT NULL CHECK (length(recipient_hpke_key_id) = 32),
    subject_device_approval_replay_id BLOB   NOT NULL CHECK (length(subject_device_approval_replay_id) = 16),
    hpke_suite_id                    INTEGER NOT NULL CHECK (hpke_suite_id BETWEEN 0 AND 9223372036854775807),
    hpke_mode_id                     INTEGER NOT NULL CHECK (hpke_mode_id BETWEEN 0 AND 9223372036854775807),
    hpke_kem_id                      INTEGER NOT NULL CHECK (hpke_kem_id BETWEEN 0 AND 9223372036854775807),
    hpke_kdf_id                      INTEGER NOT NULL CHECK (hpke_kdf_id BETWEEN 0 AND 9223372036854775807),
    hpke_aead_id                     INTEGER NOT NULL CHECK (hpke_aead_id BETWEEN 0 AND 9223372036854775807),
    hpke_info_bytes                  BLOB    NOT NULL,
    hpke_encapped_key_bytes          BLOB    NOT NULL,
    hpke_wrapped_trk_bytes           BLOB    NOT NULL,
    server_instance_id               BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch                    INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    signature_bytes                  BLOB    NOT NULL CHECK (length(signature_bytes) = 64),
    issuer_signing_key_id            BLOB    NOT NULL CHECK (length(issuer_signing_key_id) = 32),
    signed_container_hash            BLOB    NOT NULL CHECK (length(signed_container_hash) = 32),
    exact_signed_container_bytes     BLOB    NOT NULL,
    exact_signed_container_bytes_sha256 BLOB NOT NULL CHECK (length(exact_signed_container_bytes_sha256) = 32),
    revoked_at                       TEXT,
    created_at                       TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, payload_domain, replay_id),
    FOREIGN KEY (tenant_id, subject_device_id) REFERENCES v2_devices(tenant_id, id) ON DELETE CASCADE
);

-- Fleets and profiles.
CREATE TABLE v2_fleets (
    id         BLOB NOT NULL CHECK (length(id) = 16),
    tenant_id  BLOB NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    status     TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, name)
);

CREATE TABLE v2_profiles (
    id              BLOB    NOT NULL CHECK (length(id) = 16),
    tenant_id       BLOB    NOT NULL,
    fleet_id        BLOB    NOT NULL,
    name            TEXT    NOT NULL,
    current_version INTEGER NOT NULL DEFAULT 0 CHECK (current_version BETWEEN 0 AND 9223372036854775807),
    status          TEXT    NOT NULL CHECK (status IN ('active', 'archived')),
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, fleet_id) REFERENCES v2_fleets(tenant_id, id) ON DELETE CASCADE,
    UNIQUE (tenant_id, fleet_id, name)
);

-- Exclusive checkout leases with monotonic fencing tokens. The fencing token
-- is what makes a stale lease holder harmless: a delayed write carrying an old
-- token is rejected even though the lease once was valid.
CREATE TABLE v2_leases (
    id                 BLOB    NOT NULL CHECK (length(id) = 16),
    tenant_id          BLOB    NOT NULL,
    profile_id         BLOB    NOT NULL,
    holder_account_id  BLOB    NOT NULL CHECK (length(holder_account_id) = 16),
    holder_device_id   BLOB    NOT NULL CHECK (length(holder_device_id) = 16),
    fencing_token      INTEGER NOT NULL CHECK (fencing_token BETWEEN 0 AND 9223372036854775807),
    base_version       INTEGER NOT NULL CHECK (base_version BETWEEN 0 AND 9223372036854775807),
    server_instance_id BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch      INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    acquired_at        TEXT    NOT NULL,
    expires_at         TEXT    NOT NULL,
    released_at        TEXT,
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, profile_id) REFERENCES v2_profiles(tenant_id, id) ON DELETE CASCADE
);

-- At most one live lease per profile. A partial unique index expresses this
-- directly, so a double-checkout race is a constraint violation rather than
-- something the application has to notice.
CREATE UNIQUE INDEX v2_leases_one_live_per_profile
    ON v2_leases (tenant_id, profile_id)
    WHERE released_at IS NULL;

-- Signed snapshot manifests. Immutable once written: a version is committed
-- exactly once, and the exact signed bytes are what future verification reads.
CREATE TABLE v2_snapshot_manifests (
    tenant_id                        BLOB    NOT NULL,
    profile_id                       BLOB    NOT NULL,
    version                          INTEGER NOT NULL CHECK (version BETWEEN 0 AND 9223372036854775807),
    snapshot_id                      BLOB    NOT NULL CHECK (length(snapshot_id) = 16),
    fleet_id                         BLOB    NOT NULL CHECK (length(fleet_id) = 16),
    base_version                     INTEGER NOT NULL CHECK (base_version BETWEEN 0 AND 9223372036854775807),
    key_generation                   INTEGER NOT NULL CHECK (key_generation BETWEEN 0 AND 9223372036854775807),
    restore_epoch                    INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    server_instance_id               BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    fencing_token                    INTEGER NOT NULL CHECK (fencing_token BETWEEN 0 AND 9223372036854775807),
    intent_hash                      BLOB    NOT NULL CHECK (length(intent_hash) = 32),
    container_sha256                 BLOB    NOT NULL CHECK (length(container_sha256) = 32),
    container_size                   INTEGER NOT NULL CHECK (container_size BETWEEN 0 AND 9223372036854775807),
    blob_path                        TEXT    NOT NULL,
    author_account_id                BLOB    NOT NULL CHECK (length(author_account_id) = 16),
    author_device_id                 BLOB    NOT NULL CHECK (length(author_device_id) = 16),
    signature_bytes                  BLOB    NOT NULL CHECK (length(signature_bytes) = 64),
    issuer_signing_key_id            BLOB    NOT NULL CHECK (length(issuer_signing_key_id) = 32),
    signed_container_hash            BLOB    NOT NULL CHECK (length(signed_container_hash) = 32),
    exact_signed_container_bytes     BLOB    NOT NULL,
    exact_signed_container_bytes_sha256 BLOB NOT NULL CHECK (length(exact_signed_container_bytes_sha256) = 32),
    created_at                       TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, profile_id, version),
    FOREIGN KEY (tenant_id, profile_id) REFERENCES v2_profiles(tenant_id, id) ON DELETE CASCADE
);

-- Idempotency ledger for mutating operations.
--
-- Bound to (server_instance_id, restore_epoch) per plan V5.1: after a restore
-- the epoch advances, so a replayed request from before the restore does not
-- match a stored response and cannot be served a stale result.
--
-- The stored response is kept as exact bytes with its hash, so a replay
-- returns byte-identical output rather than a re-computed one that might
-- differ.
CREATE TABLE v2_operations (
    tenant_id              BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    idempotency_key        BLOB    NOT NULL CHECK (length(idempotency_key) = 16),
    server_instance_id     BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch          INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    account_id             BLOB    NOT NULL CHECK (length(account_id) = 16),
    device_id              BLOB    NOT NULL CHECK (length(device_id) = 16),
    operation_kind         TEXT    NOT NULL,
    request_sha256         BLOB    NOT NULL CHECK (length(request_sha256) = 32),
    status                 TEXT    NOT NULL CHECK (status IN ('in_flight', 'succeeded', 'failed')),
    response_status_code   INTEGER CHECK (response_status_code IS NULL OR response_status_code BETWEEN 0 AND 9223372036854775807),
    exact_response_bytes   BLOB,
    response_sha256        BLOB CHECK (response_sha256 IS NULL OR length(response_sha256) = 32),
    created_at             TEXT    NOT NULL,
    completed_at           TEXT,
    PRIMARY KEY (tenant_id, idempotency_key, server_instance_id, restore_epoch)
);

-- Resumable upload sessions, bound to instance and epoch for the same reason
-- as the operations ledger.
CREATE TABLE v2_upload_sessions (
    id                 BLOB    NOT NULL CHECK (length(id) = 16),
    tenant_id          BLOB    NOT NULL,
    profile_id         BLOB    NOT NULL,
    lease_id           BLOB    NOT NULL CHECK (length(lease_id) = 16),
    server_instance_id BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch      INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    fencing_token      INTEGER NOT NULL CHECK (fencing_token BETWEEN 0 AND 9223372036854775807),
    target_version     INTEGER NOT NULL CHECK (target_version BETWEEN 0 AND 9223372036854775807),
    intent_hash        BLOB    NOT NULL CHECK (length(intent_hash) = 32),
    declared_size      INTEGER NOT NULL CHECK (declared_size BETWEEN 0 AND 9223372036854775807),
    received_size      INTEGER NOT NULL DEFAULT 0 CHECK (received_size BETWEEN 0 AND 9223372036854775807),
    staging_path       TEXT    NOT NULL,
    status             TEXT    NOT NULL CHECK (status IN ('open', 'committed', 'aborted')),
    created_at         TEXT    NOT NULL,
    updated_at         TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, id),
    FOREIGN KEY (tenant_id, profile_id) REFERENCES v2_profiles(tenant_id, id) ON DELETE CASCADE
);

-- Signing keys a tenant currently trusts to issue authorization records.
--
-- Revocation is expressed by setting `revoked_at` rather than deleting the
-- row: the audit trail must still explain records that verified in the past.
-- Verification filters on `revoked_at IS NULL`, so a revoked issuer stops
-- being accepted without a separate check on the request path.
CREATE TABLE v2_tenant_issuers (
    tenant_id      BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    signing_key_id BLOB    NOT NULL CHECK (length(signing_key_id) = 32),
    public_key     BLOB    NOT NULL CHECK (length(public_key) = 32),
    added_at       TEXT    NOT NULL,
    revoked_at     TEXT,
    PRIMARY KEY (tenant_id, signing_key_id)
);

-- Consumption ledger for one-shot authorization records.
--
-- Separate from the record tables above, which hold the full parsed record
-- and are written when a record is *issued or stored*. This table records
-- that a record has been *used*, which is a distinct event: a record may be
-- stored and never presented, and the primary key here is what makes the
-- second presentation of the same record fail.
--
-- Keyed by (tenant, domain, replay_id): scoping by tenant means one tenant
-- cannot burn another's replay ids, and scoping by domain means an approval
-- and a grant that happen to share an id do not collide.
CREATE TABLE v2_replay_ledger (
    tenant_id             BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    payload_domain        TEXT    NOT NULL,
    replay_id             BLOB    NOT NULL CHECK (length(replay_id) = 16),
    record_table          TEXT    NOT NULL CHECK (record_table IN ('v2_device_approvals', 'v2_capability_grants')),
    signed_container_hash BLOB    NOT NULL CHECK (length(signed_container_hash) = 32),
    issuer_signing_key_id BLOB    NOT NULL CHECK (length(issuer_signing_key_id) = 32),
    not_before_ms         INTEGER NOT NULL CHECK (not_before_ms BETWEEN 0 AND 9223372036854775807),
    not_after_ms          INTEGER NOT NULL CHECK (not_after_ms BETWEEN 0 AND 9223372036854775807),
    consumed_at           TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, payload_domain, replay_id)
);

CREATE INDEX v2_devices_by_account   ON v2_devices (tenant_id, account_id);
CREATE INDEX v2_profiles_by_fleet    ON v2_profiles (tenant_id, fleet_id);
CREATE INDEX v2_manifests_by_profile ON v2_snapshot_manifests (tenant_id, profile_id, version DESC);
CREATE INDEX v2_leases_by_profile    ON v2_leases (tenant_id, profile_id);
CREATE INDEX v2_uploads_by_profile   ON v2_upload_sessions (tenant_id, profile_id, status);
