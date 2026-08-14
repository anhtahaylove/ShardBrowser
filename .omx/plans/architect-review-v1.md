# Architect review v1 — ShardX v0.2.x Team/Fleet Sync

Verdict: **REVISE**

## Strengths retained

- Keep the additive `/v2` control plane instead of tenantizing v1 in place.
- Keep profile JSON/user-data as the local source of truth.
- Replace whole-buffer plaintext snapshots with streaming encrypted backup and
  encrypted spool files.
- Keep lease + monotonic fencing token + exact `base_version`.
- Quarantine legacy plaintext v1 state; do not claim it is ciphertext-only.
- Preserve the exact v0.1.28 96-tool MCP contract as a hard gate.

## P0 repairs required before Critic

### 1. Correct the trust model

The server process is trusted for authorization and coordination. DB/blob/log
storage is not trusted for profile confidentiality or artifact integrity. A
fully malicious active coordinator can equivocate, issue two leases, or cause
DoS; preventing that requires transparency/consensus and is outside v0.2.x.
Clients still fail closed on key substitution, artifact tampering, downgrade,
and rollback relative to previously observed signed state.

### 2. Add a signing/authenticity plane

Encryption keys do not prove identity or authorize grants. Add:

- a device signing key and a distinct HPKE recipient key, each with `key_id`;
- enrollment challenge and proof-of-possession;
- out-of-band fingerprint confirmation for first owner/root bootstrap;
- owner/root-signed device approvals and fleet-key grants;
- grant claims bound to tenant, fleet, generation, device key, capability, and
  validity window;
- signed snapshot manifest/head checkpoint or hash-chain to detect substitution
  and rollback;
- no reuse of TRK/FKEK as signing keys.

### 3. Add tenant RBAC and root-custodian capabilities

Use deny-by-default `owner`, `admin`, and `member` roles plus explicit
capabilities. Root custody is a separate capability. Ordinary fleet devices do
not receive the tenant root key. Approval, revoke, rotate, recovery, and forced
lease expiration must check capabilities in the same transaction and fail if
the audit event cannot be persisted.

### 4. Make blob + SQLite crash consistency executable

PATCH state machine:

1. require exact committed offset;
2. write bytes to the encrypted staging file;
3. `sync_data` the file;
4. advance committed offset in SQLite;
5. after restart, truncate a longer file tail to the committed offset and
   quarantine a file shorter than the committed offset.

Finalize/commit state machine:

1. stream-recompute final ciphertext hash and size;
2. rename to an immutable content-addressed path;
3. fsync the file and parent directory;
4. in a SQLite CAS transaction, insert snapshot, advance profile version,
   release lease, and persist the exact idempotent receipt;
5. a DB failure leaves an unreferenced immutable ciphertext object for
   fail-closed reconciliation/GC;
6. inject crashes before/after write, fsync, offset update, rename, DB commit,
   and response.

### 5. Remove unsafe downgrade language

Rolling back the binary to v0.1.28 can bypass Team lease guards because v0.1.28
does not understand `team-sync.db`. Downgrade is allowed only when no active
lease, pending operation, restore journal, or team binding remains; all bindings
must be explicitly unbound/cloned to a new local profile, or the complete
pre-v2 config/profile/user-data backup must be restored. Track
`server_instance_id` and `restore_epoch` and quarantine state after server
rollback. Never say an old binary may simply ignore the Team DB.

## Contract-level P1 repairs

1. Define one immutable bounded envelope preamble and authenticated slot table;
   bind recipient key ID, generation, suite, context, manifest commitment, and
   canonical critical fields. Reject trailing bytes, zero frames, missing or
   repeated final frame, counter exhaustion, and non-canonical encodings.
2. Model one current lease row per profile. Renew requires
   `expires_at > server_now`. Offline grace only permits an already-running
   browser to continue with warning; no start/relaunch or remote commit after
   expiry.
3. Key generations use `PREPARING -> ACTIVE -> RETIRED`; activate only after a
   recovery slot and every required device slot are read back. Revoke and new
   generation activation are one logical operation.
4. Persist commit idempotency request/receipt for at least snapshot retention;
   canonicalize request hash and operation scope. Offset mismatch defaults to
   HEAD/resume; duplicate chunks require an explicit digest.
5. Provide executable local `team-sync.db` DDL invariants: PK/FK/UNIQUE/CHECK,
   canonical server origin, singleton schema row, fail-closed unknown/newer
   schema, unique local and remote profile bindings, and recovery journals.
6. Add strict v2 archive validation. Unsupported entries, duplicate normalized
   paths, case-fold collisions, ADS/reserved names, and file/dir conflicts fail
   before swap. Preserve v1 behavior to avoid regression.
7. Capture a canonical v0.1.28 fixture for all 96 MCP tools including names,
   descriptions, annotations, and input schemas. Add `server/openapi.yaml`,
   schema fixture, contract tests, and stable HTTP/error mapping to impact map.
8. Make dependency/security spike a pre-phase hard gate. Treat v0.2.0 as an
   internal foundation, not a production release. Windows is the first Team
   runtime; macOS/Linux remain local-only until credential-store platform tests
   pass.

## P2 clarifications

- Accepted plaintext metadata includes server origin or tenant locator,
  ciphertext sizes, timing, opaque IDs, and membership relationships. Labels
  remain encrypted/randomized.
- Add aggregate tenant quota, concurrent upload limit, chunk cap, encrypted
  spool reserve, and minimum free-disk preflight. Keep 512 MiB as pilot
  per-snapshot default pending fixtures.
- Map each stable error code to HTTP status, retry policy, `Retry-After`, and
  client state transition.

## Open-question decisions

- Key hierarchy: accept TRK -> FKEK -> DEK after adding signed grants; only root
  custodians receive TRK.
- STREAM/XChaCha/HPKE: provisional candidate only; dependency spike must prove
  API, final-frame semantics, vectors, MSRV, maintenance, and platform support.
- Metadata privacy: labels encrypted; routing/timing/size leakage documented.
- Snapshot quota: 512 MiB pilot plus aggregate/concurrency/disk-reserve guards.
- Lease partition: do not kill a running browser; after expiry it is an
  `offline_fork`, with no relaunch or remote commit.
- Legacy v1: quarantine + remote-off, no automatic migration/scrub in early
  v0.2.x.
- Production release: blocked until a named operator completes verified backup,
  rollback, and recovery-bundle readback drills.

## Required next step

Revise the main plan and open questions with every P0 and contract-level P1
repair, then return it to Architect before any Critic review.
