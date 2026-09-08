-- Fleet key encryption key (FKEK) generations and their device grants.
--
-- Profile sync encrypts with a per-fleet key, not the tenant root key. The
-- root key's job is to authorise custody; the FKEK is what actually wraps a
-- snapshot's DEK. Deriving one from the other would collapse two lifetimes
-- into one: rotating a fleet key would force a root rotation, and a device
-- removed from a fleet would still hold the material to decrypt it.
--
-- Scope: the DeviceHpkeGrant path from plan 5.6. RecoveryGrant and
-- RotationGrant are specified there and are not implemented here, so their
-- columns are deliberately absent rather than present and unused. The variant
-- CHECK still lists all three: it is the shape the wire contract fixes, and a
-- narrower list would have to be rebuilt to widen later.

CREATE TABLE v2_fleet_key_generations (
    tenant_id     BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    fleet_id      BLOB    NOT NULL,
    generation    INTEGER NOT NULL CHECK (generation BETWEEN 0 AND 9223372036854775807),
    fkek_key_id   BLOB    NOT NULL CHECK (length(fkek_key_id) = 32),

    -- The root generation whose custodian issued this fleet key. Recorded so a
    -- fleet key can be traced to the root that authorised it, which is what a
    -- later recovery grant has to check.
    root_generation INTEGER NOT NULL CHECK (root_generation BETWEEN 0 AND 9223372036854775807),

    -- PREPARING accepts grants; ACTIVE is the only state sync may encrypt
    -- under; RETIRED is kept so existing snapshots stay attributable.
    state         TEXT    NOT NULL CHECK (state IN ('PREPARING', 'ACTIVE', 'RETIRED')),

    created_at    TEXT    NOT NULL,
    activated_at  TEXT,
    retired_at    TEXT,

    PRIMARY KEY (tenant_id, fleet_id, generation),

    -- A fleet key belongs to exactly one generation, for the same reason a
    -- root key does: the generation number is how a reader decides which key
    -- wrapped a snapshot, and reuse would make that ambiguous.
    UNIQUE (tenant_id, fkek_key_id),

    FOREIGN KEY (tenant_id, fleet_id) REFERENCES v2_fleets(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_v2_fleet_key_generations_state
    ON v2_fleet_key_generations (tenant_id, fleet_id, state);

-- At most one ACTIVE generation per fleet. Two active generations would mean
-- two valid answers to "which key do I encrypt under", and devices holding
-- different grants would write snapshots the others cannot read.
CREATE UNIQUE INDEX idx_v2_fleet_key_generations_one_active
    ON v2_fleet_key_generations (tenant_id, fleet_id)
    WHERE state = 'ACTIVE';

-- Signed fleet key grants, stored outside any envelope so adding or revoking a
-- device never rewrites a snapshot.
--
-- Column-per-field mirrors the decoded payload one-for-one, as the root grant
-- table does: the exact signed bytes are the authority, and the columns exist
-- so the server can enforce and index without parsing on every read.
CREATE TABLE v2_fleet_key_grants (
    tenant_id                        BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    replay_id                        BLOB    NOT NULL CHECK (length(replay_id) = 16),
    payload_domain                   TEXT    NOT NULL,
    grant_variant                    TEXT    NOT NULL CHECK (grant_variant IN ('FirstFleetSelfGrant', 'DeviceHpkeGrant')),

    fleet_id                         BLOB    NOT NULL,
    fkek_key_id                      BLOB    NOT NULL CHECK (length(fkek_key_id) = 32),
    fleet_generation                       INTEGER NOT NULL CHECK (fleet_generation BETWEEN 0 AND 9223372036854775807),
    grant_capability                 TEXT    NOT NULL,

    -- DeviceHpkeGrant fields. Nullable because the other two variants have no
    -- subject; application code requires them for this variant.
    subject_account_id               BLOB    CHECK (subject_account_id IS NULL OR length(subject_account_id) = 16),
    subject_device_id                BLOB    CHECK (subject_device_id IS NULL OR length(subject_device_id) = 16),
    subject_signing_key_id           BLOB    CHECK (subject_signing_key_id IS NULL OR length(subject_signing_key_id) = 32),
    recipient_hpke_key_id            BLOB    CHECK (recipient_hpke_key_id IS NULL OR length(recipient_hpke_key_id) = 32),
    hpke_suite_id                    INTEGER CHECK (hpke_suite_id IS NULL OR hpke_suite_id BETWEEN 0 AND 9223372036854775807),
    hpke_info_bytes                  BLOB,
    hpke_encapped_key_bytes          BLOB,
    hpke_wrapped_fkek_bytes          BLOB,

    server_instance_id               BLOB    NOT NULL CHECK (length(server_instance_id) = 16),
    restore_epoch                    INTEGER NOT NULL CHECK (restore_epoch BETWEEN 0 AND 9223372036854775807),
    issued_at_ms                     INTEGER NOT NULL CHECK (issued_at_ms BETWEEN 0 AND 9223372036854775807),
    not_before_ms                    INTEGER NOT NULL CHECK (not_before_ms BETWEEN 0 AND 9223372036854775807),
    not_after_ms                     INTEGER NOT NULL CHECK (not_after_ms BETWEEN 0 AND 9223372036854775807),

    signature_bytes                  BLOB    NOT NULL CHECK (length(signature_bytes) = 64),
    issuer_signing_key_id            BLOB    NOT NULL CHECK (length(issuer_signing_key_id) = 32),
    signed_container_hash            BLOB    NOT NULL CHECK (length(signed_container_hash) = 32),
    exact_signed_container_bytes     BLOB    NOT NULL,
    exact_signed_container_bytes_sha256 BLOB NOT NULL CHECK (length(exact_signed_container_bytes_sha256) = 32),

    revoked_at                       TEXT,
    created_at                       TEXT    NOT NULL,

    PRIMARY KEY (tenant_id, payload_domain, replay_id),

    -- A device gets at most one live grant per generation. Without this a
    -- second grant could wrap a different key under the same generation, and
    -- which one a device picked up would decide whether its snapshots were
    -- readable. Revoked rows are excluded so a device can be re-granted after
    -- a revocation.
    UNIQUE (tenant_id, fleet_id, fleet_generation, subject_device_id)
);

-- Retrieval is always "grants for this device"; without this index that read
-- is a scan of every grant in the tenant.
CREATE INDEX idx_v2_fleet_key_grants_device
    ON v2_fleet_key_grants (tenant_id, subject_device_id, revoked_at);

CREATE INDEX idx_v2_fleet_key_grants_generation
    ON v2_fleet_key_grants (tenant_id, fleet_id, fleet_generation);
