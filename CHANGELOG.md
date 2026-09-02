# Changelog

## Unreleased

### Fleet transfer client and a commit-path authorization fix

Adds the `/v2/` client from #17 and fixes a server flaw the work exposed.

- Add `shared::fleet_manifest`: one definition of the signed snapshot manifest,
  used by both the client that signs and the tests that verify, so the two
  cannot drift into a format only one side accepts.
- Add `fleet_client` in the Launcher: lease, chunked upload, signed commit and
  ranged download. The lease is released on every path, a failed upload aborts
  its staging session, and a download that stalls is an error rather than a
  silent truncation.
- Add `GET /v2/server-identity`. A client must bind its signed records to the
  live `server_instance_id` and `restore_epoch`; without this endpoint it could
  only guess them.
- **Security fix.** `/v2/fleet/uploads/commit` verified the manifest signature
  and then trusted the request body for `container_sha256`, `profile_id`,
  `snapshot_id`, `fleet_id`, `base_version` and `key_generation`. A caller with
  a valid token could present a genuine manifest signed for one snapshot and
  publish different bytes under it. The handler now rejects any body that
  contradicts the signed manifest, covered by a regression test that publishes
  version 1 when the check is removed.

`fleet_client` has no UI caller yet: that needs device enrollment, which the
server does not expose an endpoint for. #17 stays open.

### Encrypted profile backup, wired into the Launcher

Closes the gap recorded in #17: v0.2.0 shipped the sealing library and the
fleet server, but nothing in the app could reach them. `shared::backup` now has
a production caller.

- Add `shared::passphrase`: Argon2id (64 MiB, 3 passes) derivation of the
  backup FKEK from a user passphrase. Nothing is persisted but a random salt,
  so a backup opens on a machine that has never seen the source.
- Add `shared::backup_file`: `create`/`restore`/`inspect` over a self-contained
  `.shxbak` file (magic, salt, verifying key, sealed container). Written via a
  temp file and renamed, so an interrupted backup cannot replace a good file
  with a truncated one.
- Add Tauri commands `profile_backup_create`, `profile_backup_restore` and
  `profile_backup_inspect`. Both mutating commands take the same
  `begin_user_mutation` claim as delete/clone: a backup of a running profile is
  torn, and a restore under one corrupts it.
- Add "Back up (encrypted)" and "Restore from backup" to the profile menu, with
  a passphrase prompt that requires confirmation on backup and warns that a lost
  passphrase makes the backup unreadable.

Restore recovers and authenticates the whole container in memory before
`snapshot::unpack` touches the profile. `backup::open` streams authenticated
frames and only detects truncation at the signed head, so an in-place restore
could otherwise leave a profile that looks complete and was never verified.

## 0.2.0 - 2026-09-02

> Scope: this release adds a library and a server. It is not wired into the
> app — `shared::backup::{seal, open}` has no production caller, and the
> Launcher exposes no backup, restore, fleet or sync command. The entries
> below describe implemented and tested building blocks, not user-facing
> features.

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
