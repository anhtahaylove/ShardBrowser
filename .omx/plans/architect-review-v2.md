# Architect review v2 — remaining blockers

Verdict: **REVISE**

The first revision closed findings 1, 3, 7-9, and 11-16. The remaining
blockers are contract inconsistencies that must be fixed before Critic review.

## 17. Break the cryptographic commitment cycle

The critical header/AAD cannot include a commitment to a final signed manifest
that itself contains the ciphertext hash. Use two canonical records:

1. `EnvelopeIntentV2` is created before encryption and excludes ciphertext hash
   and size. Frames bind `intent_hash` as AAD.
2. After encryption, `SnapshotManifestV2` is created and signed. It contains
   `intent_hash`, exact header/slot hashes, ciphertext hash and size, previous
   signed head, signer key ID, server instance, restore epoch, and version.

The encrypted critical header must not contain a final-manifest commitment.

## 18. Use one authoritative slot model

Keep the approved hierarchy:

- TRK wraps/authorizes fleet key generations.
- Device HPKE grants distribute an FKEK generation outside the snapshot
  envelope.
- Snapshot envelope contains one DEK wrapping slot under immutable FKEK
  `key_id` + generation, not one DEK slot per device.

Persist immutable canonical signed grant bytes plus hash, issuer signing key ID,
recipient HPKE key ID, tenant/fleet/generation/capability, validity, server
instance ID, and restore epoch. Clients verify the signed grant bytes, not just
server columns.

## 19. Complete the upload recovery matrix

Define deterministic reconciliation for every combination of:

- DB state: `OPEN`, `FINALIZING`, `READY`, `COMMITTED`, `QUARANTINED`;
- staging object missing/present;
- immutable object missing/present;
- hash/size invalid/valid;
- snapshot receipt absent/present.

At minimum:

- `FINALIZING` + valid immutable only: fsync/re-hash then CAS to `READY`.
- `FINALIZING` + valid staging only: resume finalize.
- both valid: immutable wins only when hashes match; remove staging after DB
  state is durable.
- any short/corrupt object: `QUARANTINED`; never silently retry commit.
- `READY` + missing/invalid immutable object: `QUARANTINED` and profile head is
  not advanced.
- `COMMITTED` requires snapshot row, exact receipt, immutable object and hash;
  a mismatch is a security incident, not GC.

No `FINALIZING` row may remain indefinitely after reconciliation.

## 20. Repair local workflow DDL

- Preserve completed unbind receipts independently of `profile_bindings` using
  a receipt/tombstone table or nullable binding reference with `ON DELETE SET
  NULL`; a hard FK must not block binding deletion.
- Add `RELEASE` to the operation enum.
- Persist `remote_upload_id`, committed offset, expected ciphertext digest/size,
  exact request hash, exact response/receipt, retry state, and spool path.
- Replace singleton active generation with per-server/per-tenant/per-fleet/
  generation key-state rows.
- Add workflow DDL tests for completed UNBIND then binding delete, RELEASE
  persistence, upload resume, exact receipt replay, unknown schema rejection,
  and recovery journal replay.

## 21. Close safe downgrade and restore reconciliation

- A local clone is downgrade-safe only after the original team-bound profile
  metadata and user-data are moved outside every discovery path understood by
  v0.1.28, with a durable downgrade journal and readback.
- The clone receives a new local profile ID and contains no Team marker or
  credential artifacts.
- Complete pre-v2 restore is a separate path that restores config/profile/
  user-data and then archives/retires Team credential artifacts.
- Add a root-signed `RestoreEpochTransitionV2` that binds server instance,
  previous epoch/head, new epoch/head, reason, approver, timestamp, and nonce.
  Clients only exit rollback quarantine after verifying this transition.

## Required synchronization

Update the trust/key/envelope/schema/API/local-DB/migration/test/goal sections,
the closure matrix, and `open-questions.md`. Re-run syntax, DDL, and workflow
probes before the final Architect review.
