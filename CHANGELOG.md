# Changelog

## 0.2.0 - 2026-09-02

### Team/fleet control plane (new)

- Add `shared/src/grants.rs`: RFC 9180 HPKE (DHKEM-X25519 / HKDF-SHA256 /
  ChaCha20Poly1305) tenant root key grants. The grant scope — tenant, fleet,
  profile, snapshot, key generation, recipient device — is bound as AEAD
  associated data, so a grant cannot be replayed under a different scope.
  Sealing uses the OS CSPRNG.
- Add the v2 control-plane schema (`0004_v2_team_fleet.sql`), additive over v1.
  Tenant-scoped rows join their parent by composite `(tenant_id, id)` foreign
  key, so cross-tenant references are rejected by the database rather than
  relying on every query remembering to filter. One-shot records are claimed
  through a UNIQUE index.
- Add signed authorization records: a request must carry a record binding the
  action to this domain, tenant, server instance and restore epoch, signed by
  an issuer the tenant trusts.
- Add replay rejection and idempotent retries: a record is consumable once, and
  a completed operation replays its exact original response bytes.
- Add `/v2/device-approvals`, `/v2/capability-grants`,
  `/v2/tenant-root-key-grants`, `/v2/operations/complete`. The server files
  root key grants without being able to read them — the payload stays sealed
  to the recipient device.

### Encrypted profile backup (new)

- Add a v2 encrypted backup container: streaming ChaCha20-Poly1305 frames over a
  wrapped per-snapshot DEK, with an Ed25519-signed head record.
- Add `shared/src/canonical.rs`: deterministic canonical CBOR encoding with
  strict rejection of non-canonical input, so signed bytes cannot be re-encoded
  into a different-but-accepted form.
- Add `shared/src/keys.rs`: key identities (`root_key_id`, `signing_key_id`) and
  DEK wrap/unwrap bound to a canonical slot context as AEAD associated data.
- Add `shared/src/signing.rs`: domain-separated signing over a length-prefixed
  transcript, with an acyclic commitment structure (the signature is not part of
  its own signed payload).
- Add `shared/src/envelope.rs`: DEK slots, pre-encryption intent records, STREAM
  frame nonces/AAD, and the restore-epoch Merkle tree with inclusion proofs.
- Add `shared/src/backup.rs`: the `seal`/`open` container API joining the above.

### Format guarantees

- Frames bind the exact envelope intent, frame counter, and final-frame flag, so
  truncation, reordering, and cross-envelope splicing all fail closed.
- The prologue is bound to the signed head, so slot/intent substitution before
  the signature check is rejected.
- Every single-byte mutation of a container is detected (exhaustive test).
- Merkle leaves enforce ordering, reject duplicates, and use domain-separated
  leaf/node hashing; unary promotion is only legal at an odd tail.

### Compatibility

- The v1 portable snapshot format is unchanged: `snapshot::pack`/`unpack` keep
  their existing behaviour and byte format.
- The v2 container wraps a v1 snapshot losslessly — sealing then opening returns
  the exact v1 bytes, which still restore through the v1 reader.
- The v1 reader rejects a v2 container rather than misparsing it.
- The MCP tool contract is unchanged at 96 tools.

### Notes

- `open` streams plaintext as it is decrypted, so its output must be treated as
  untrusted until the call returns `Ok`; this is a deliberate trade-off to keep
  memory bounded on large profiles. Callers restoring to disk should write to a
  staging path and promote only on success.
- Fleet sync transfer operations are not implemented in this release: the
  schema and authorization exist, the upload/download path does not.

## 0.1.29 - 2026-08-28

### Process ownership

- Assign an opaque in-memory UUID to every API-launched browser and require the exact profile, PID, and launch-instance token for conditional cleanup.
- Disable the legacy PID-only conditional stop endpoint with HTTP 410 so recycled numeric PIDs cannot stop replacement processes.
- Keep launch-instance tokens out of running-profile inventory, persistence, logs, API errors, and public MCP results.

### MCP lifecycle

- Preserve the page opened by `safe_open_url` as the active target for follow-up tab, screenshot, ARIA, and network tools.
- Preflight Launcher ownership capability before spawning a stopped profile and fail closed when CDP startup, restoration, or launch-instance ownership cannot be verified.
- Restore the profile state owned by `safe_open_url` without PID-only retries; stale-process cleanup is now inventory-only.

### Release integrity

- Enforce exact MCP archive-entry equality across staging, raw tar inspection, and extracted payload verification.
- Reject missing, extra, duplicate, nested-extra, symlink, non-regular, unsafe-root, and malformed release-staging inputs before publication.
- Preserve the 96-tool MCP contract, startup-in-tray behavior, signed updater flow, and existing headless automation compatibility.

## 0.1.28 - 2026-08-14

### Rust SDK

- Update the crate-level quickstart doctest to create a `Profile` before calling `ShardX::session`.
- Gate the CDP-control doctest and `quickstart` example behind the existing `control` feature so `--no-default-features` remains buildable.
- Upgrade `dirs` from 5 to 6 and `rand` from 0.8 to 0.9 after isolated default-feature, no-default-feature, and Rust 1.74.1 compatibility checks.
- Keep `chromiumoxide` 0.7, `reqwest` 0.12, and `zip` 2 because their current release lines are RustSec-clean while the next majors exceed the SDK's Rust 1.74 MSRV.
- Add stable, RustSec, and Rust 1.74.1 SDK gates to CI and release validation so vulnerable dependency graphs, doctest failures, feature-minimal regressions, and MSRV breaks cannot bypass packaging.

## 0.1.27 - 2026-08-13

### Security

- Resolved the current npm audit findings in the Launcher, MCP server, and Node SDK dependency graphs with compatible patch-level updates.
- Added a Node SDK archive extraction regression test for nested Windows paths and Unicode fingerprint bundles.

### Profile safety

- Reject profile mutations while the browser is running or while launch/exit cleanup owns the profile lifecycle.
- Serialize concurrent profile-name allocation and mutation while preserving unrelated profile launches.
- Validate profile names consistently across UI, API, create, edit, clone, and batch import paths.
- Reject unsafe Windows names, separators, control characters, overlong names, and case-insensitive collisions.
- Fail closed when profile inventory contains an unreadable or malformed record.
- Persist profile records with atomic replacement and report folder-operation write/delete failures instead of returning false success.

### Compatibility

- Preserve the 96-tool MCP contract, startup-in-tray behavior, signed updater flow, and existing headless automation compatibility.
- Defer SQLite/WAL dependency migration to the Team/Fleet Sync and encrypted-backup development line because the current server does not enable WAL.
