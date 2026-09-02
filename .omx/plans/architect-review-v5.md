# Architect review v5 — final alignment repairs

Verdict: **REVISE**

No P0 remains. The core wire/security contracts are accepted. Close these four
alignment issues before Critic.

## 1. Bind replay rows to server instance and restore epoch

Add `server_instance_id` and `restore_epoch` to local `operations` and
`upload_sessions` where COMMIT/resume/replay uses them. Add CHECK/range/FK rules
and mismatch probes. The exact persisted request/container remains the replay
authority; relational columns must equal the signed/canonical bytes.

## 2. Choose one restore-epoch authority

The authoritative monotonic `server_instance_id + restore_epoch` record is a
small, checksummed, fsync'd file outside the rollback scope of the SQLite DB.
SQLite `v2_server_state` is a transactional mirror/cache only. On startup:

- external record missing/corrupt or behind DB: fail closed/quarantine;
- external record ahead of DB after a legitimate restore: require the
  tenant-scoped signed `RestoreEpochTransitionV2`, then rebuild the DB mirror;
- never lower the external epoch during DB restore;
- define write order: prepare signed transition and restored DB, fsync them,
  atomically replace+fsync the external epoch record, then open/reconcile DB.

## 3. Align wire integer ranges with SQLite

Every wire integer persisted in SQLite, including fencing token, version,
offset, size, epoch, and timestamps, is canonically encoded as unsigned but is
restricted to `0..i64::MAX`. Decoder and schema reject values above that range.
If a future field needs full U64, it must use a separately versioned big-endian
blob/text encoding and cannot silently enter SQLite INTEGER.

## 4. Remove goal/handoff drift

Formal order is:

`Planner/Architect/Critic consensus -> bounded G2 research/dependency/durability
spike -> verifier PASS -> production implementation`.

No follow-up may place G2 before Critic. The stale tracked
`docs/NEXT_V0.2_X_GOAL.md` must be replaced by a short superseded pointer to the
consensus artifact (or fully synchronized). The pointer must say runtime stays
v0.1.28 and no implementation/release starts from the old text.

## Required validation

- DDL mismatch probes for server instance/epoch in operations and uploads;
- restore DB rollback/epoch crash-order state table;
- over-i64 wire value rejection vectors;
- structural check that no handoff path places G2 before Critic;
- tracked goal pointer no longer references v0.1.27 as executable authority.
