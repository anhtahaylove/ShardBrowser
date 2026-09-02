# Architect review v3 — final contract repairs

Verdict: **REVISE**

Findings 17-19 are closed. Close findings 22-24 and the DoD wording before
Critic review.

## 22. Persist exact signed authorization claims

Every device approval, tenant capability grant, and fleet-key grant must store:

- exact canonical signed claim bytes;
- claim hash;
- signature bytes and signature suite/version;
- issuer signing key ID;
- subject account/device signing key and HPKE key IDs as applicable;
- tenant, fleet, generation, capability/scope, validity window, server instance,
  and restore epoch extracted into indexed columns;
- a contract that verifies signature and exact equality between signed bytes and
  indexed columns before authorization or key release.

The database is not an authenticity authority merely because it has indexed
columns.

## 23. Make restore-epoch reconciliation tenant/profile aware

Keep a server-global monotonic restore epoch, but use a tenant-scoped signed
`RestoreEpochTransitionV2` keyed by:

`(server_instance_id, tenant_id, previous_epoch, new_epoch)`.

The canonical transition commits to either:

- a canonical sorted set/Merkle root of previous and new profile-head mappings,
  with inclusion proofs; or
- a precisely defined tenant checkpoint that provides equivalent coverage.

The local transition cache is tenant-scoped. Each profile binding exits
rollback quarantine only after the transition proof covers that profile's
previous and new signed head. A tenant root cannot authorize another tenant's
transition.

## 24. Persist exact commit request for restart replay

Before finalize/commit, durably persist either:

- exact signed `SnapshotManifestV2` bytes; or
- an immutable sidecar path plus hash/size, with fsync ordering.

The local operation row binds this exact artifact to idempotency key, canonical
request hash, upload ID, lease/fence/base version, intent hash, ciphertext hash,
and expected exact receipt. After close/reopen, retry must replay byte-identical
request content or fail closed. Add workflow probes for crash before request,
after request before response, and after response before local receipt commit.

## 25. Clarify definition of done

Implementation gates G0-G7 must pass for a completed implementation handoff.
The production-operator gate may remain blocked, but in that state the artifact
must be labeled non-production-ready and no release/tag/publish/production
migration is allowed. Production-ready requires all gates including named
operator and recovery/rollback drills.

## Required synchronization

Update server/local schema, trust/auth, restore, tests, gate matrix, closure
matrix, goal prompt, and open questions. Add read-only structural probes for:

- exact claim bytes versus indexed columns;
- close/reopen byte-identical manifest replay;
- multi-tenant/multi-profile transition coverage and cross-tenant rejection.
