-- Root key generation lifecycle.
--
-- A grant names a root generation, but nothing recorded which generations
-- exist or which one is in force. A grant could therefore reference a
-- generation that was never created, and a device had no way to learn which
-- generation to use.
--
-- Scope: the bootstrap path only. A tenant creates generation 0 in PREPARING,
-- files the first self-grant, and activates once the holder confirms it can
-- unwrap. Rotation and recovery bundles are specified in plan 5.6 but are not
-- implemented here, so the columns they need are deliberately absent rather
-- than present and unused.

CREATE TABLE v2_root_key_generations (
    tenant_id     BLOB    NOT NULL REFERENCES v2_tenants(id) ON DELETE CASCADE,
    generation    INTEGER NOT NULL CHECK (generation BETWEEN 0 AND 9223372036854775807),
    root_key_id   BLOB    NOT NULL CHECK (length(root_key_id) = 32),

    -- PREPARING accepts the self-grant; ACTIVE is the only state a sync may
    -- use; RETIRED is kept so historical snapshots remain attributable.
    state         TEXT    NOT NULL CHECK (state IN ('PREPARING', 'ACTIVE', 'RETIRED')),

    created_at    TEXT    NOT NULL,
    activated_at  TEXT,
    retired_at    TEXT,

    PRIMARY KEY (tenant_id, generation),

    -- A root key belongs to exactly one generation. Reusing a key across
    -- generations would make the generation number meaningless for deciding
    -- which key sealed a snapshot.
    UNIQUE (tenant_id, root_key_id)
);

-- Grants reference a generation; without this a grant could name a generation
-- that does not exist. Enforced in application code rather than as a foreign
-- key because the grants table predates this one and SQLite cannot add a
-- foreign key to an existing table without a full rebuild.
CREATE INDEX idx_v2_root_key_generations_state
    ON v2_root_key_generations (tenant_id, state);

-- At most one generation may be ACTIVE per tenant. Two active generations
-- would leave "which key seals a new snapshot" undefined, and a partial index
-- states that in the schema rather than trusting every write path to check.
CREATE UNIQUE INDEX idx_v2_root_key_generations_one_active
    ON v2_root_key_generations (tenant_id)
 WHERE state = 'ACTIVE';

-- At most one FirstRootSelfGrant per tenant, as plan 5.6 requires. The
-- bootstrap transaction also checks emptiness; this index is the backstop that
-- holds even if a future caller forgets, and it is the constraint that makes
-- "the first device defines custody" true rather than merely intended.
--
-- Upgrade note: v0.2.3 stored grants without this constraint, so a deployment
-- that filed two self-grants for one tenant will fail this migration and the
-- server will not start. That is deliberate. Such a tenant has two competing
-- claims to define custody, and picking one automatically would silently
-- discard somebody's root key. Resolving it requires deciding which grant is
-- legitimate, which is an operator decision, not a migration's.
CREATE UNIQUE INDEX idx_v2_tenant_root_key_grants_one_self_grant
    ON v2_tenant_root_key_grants (tenant_id)
 WHERE grant_variant = 'FirstRootSelfGrant';
