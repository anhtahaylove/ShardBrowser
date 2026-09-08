-- Allow fleet key grants into the replay ledger.
--
-- Same failure mode as migration 0005, which had to widen this CHECK for root
-- key grants: the claim runs as `INSERT OR IGNORE ... changes()`, and OR
-- IGNORE swallows a CHECK violation exactly like a duplicate primary key —
-- zero rows affected. The endpoint reads that as "already used" and answers
-- 409, so filing a grant looks like a replay of a record that was never
-- stored, and the grant gets no replay protection at all.
--
-- SQLite cannot alter a CHECK in place, so the table is rebuilt. Existing rows
-- are preserved: the ledger is what makes replays detectable, and dropping it
-- would let every previously consumed record be presented again.

PRAGMA foreign_keys = OFF;

CREATE TABLE v2_replay_ledger_new (
    tenant_id             BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    payload_domain        TEXT    NOT NULL,
    replay_id             BLOB    NOT NULL CHECK (length(replay_id) = 16),
    record_table          TEXT    NOT NULL CHECK (record_table IN ('v2_device_approvals', 'v2_capability_grants', 'v2_tenant_root_key_grants', 'v2_fleet_key_grants')),
    signed_container_hash BLOB    NOT NULL CHECK (length(signed_container_hash) = 32),
    issuer_signing_key_id BLOB    NOT NULL CHECK (length(issuer_signing_key_id) = 32),
    not_before_ms         INTEGER NOT NULL CHECK (not_before_ms BETWEEN 0 AND 9223372036854775807),
    not_after_ms          INTEGER NOT NULL CHECK (not_after_ms BETWEEN 0 AND 9223372036854775807),
    consumed_at           TEXT    NOT NULL,
    PRIMARY KEY (tenant_id, payload_domain, replay_id)
);

INSERT INTO v2_replay_ledger_new (
    tenant_id, payload_domain, replay_id, record_table,
    signed_container_hash, issuer_signing_key_id,
    not_before_ms, not_after_ms, consumed_at
)
SELECT
    tenant_id, payload_domain, replay_id, record_table,
    signed_container_hash, issuer_signing_key_id,
    not_before_ms, not_after_ms, consumed_at
FROM v2_replay_ledger;

DROP TABLE v2_replay_ledger;

ALTER TABLE v2_replay_ledger_new RENAME TO v2_replay_ledger;

PRAGMA foreign_keys = ON;
