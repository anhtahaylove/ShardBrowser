# Goal: ShardBrowser v0.2.x Team/Fleet Sync and encrypted profile backup

Execute this goal end-to-end on a new `codex/v0.2.x-team-fleet-encrypted-backup` branch after v0.1.27 is installed and verified. Do not reuse or publish the provenance-foreign `v0.2.0` tag.

## Outcome

Deliver an opt-in Team/Fleet Sync and encrypted profile-backup line that preserves local-first operation, profile isolation, the 96-tool MCP contract, startup-in-tray behavior, and existing automation compatibility. No migration or release is allowed without tested restore and rollback paths.

## Phase 1 - Baseline and threat model

1. Re-fetch `origin`, push-disabled `upstream`, published releases, dependency audits, and current runtime versions.
2. Inventory `server`, `shared`, Launcher sync/snapshot callers, SQLite versions, migrations, transaction boundaries, and any existing checkout locks.
3. Write a threat model covering server compromise, stolen backup files, replay, cross-tenant access, key loss, partial upload/download, concurrent checkout, and rollback.
4. Define explicit non-goals and compatibility guarantees before implementation.

## Phase 2 - SQLite and migration safety

1. Add migration tests from every supported database schema version before changing SQLite dependencies or journal mode.
2. Evaluate the targeted `sqlx`, `rusqlite`, and bundled SQLite updates independently; do not run a broad `cargo update`.
3. Introduce WAL only after tests prove checkpoint, busy-timeout, crash recovery, backup consistency, and downgrade/rollback behavior.
4. Fail closed on unknown/newer schemas and preserve a verified pre-migration backup.

## Phase 3 - Team/Fleet Sync

1. Implement tenant-scoped users, roles, fleet membership, profile checkout leases, renewals, stale-lock takeover, and audit events.
2. Make every write idempotent and versioned; detect conflicts instead of last-write-wins overwrite.
3. Keep local profiles usable when the team server is unavailable; queue only operations proven safe to replay.
4. Add API/MCP surfaces only after authorization, schema, error, rate-limit, and redaction contracts are tested.

## Phase 4 - Encrypted profile backup

1. Encrypt each backup before upload using authenticated encryption and a versioned envelope; never store plaintext profile data, cookies, proxy credentials, or keys server-side.
2. Separate encryption keys from backup storage, support key rotation, and provide an explicit recovery-key export flow with clear loss semantics.
3. Bind manifest, profile identity, version, and archive contents into authenticated metadata to prevent substitution and replay.
4. Stream archives with bounded memory and reject path traversal, symlinks, decompression bombs, malformed manifests, and partial files.
5. Restore into staging, verify integrity and schema, then atomically swap; preserve a rollback copy until post-restore smoke passes.

## Required tests

- Unit and property tests for authorization, lease races, idempotency, encryption envelopes, archive validation, and conflict detection.
- Migration matrix for old/current/future schema rejection plus WAL crash/checkpoint recovery.
- Server/shared integration tests for concurrent clients, interrupted uploads/downloads, retries, stale locks, and tenant isolation.
- Restore tests for Windows paths, Unicode names, cookies/storage, large profiles, wrong keys, corrupted archives, and rollback.
- Launcher/API/MCP E2E tests proving local-only behavior remains unchanged when sync is disabled or unavailable.
- Upgrade smoke from the latest v0.1.x release with settings, startup-in-tray, MCP tools, and canonical profiles preserved.

## Release gates

- All dependency audits, fmt, clippy, unit, integration, E2E, migration, restore, installer, signature, and updater checks pass.
- Independent security review finds no cross-tenant, plaintext-secret, path traversal, replay, or rollback gap.
- A disposable test fleet completes backup, restore, conflict, offline, and recovery-key drills without using the canonical local profile.
- Release only once all gates pass; otherwise keep the branch reviewable and publish no tag or artifact.
