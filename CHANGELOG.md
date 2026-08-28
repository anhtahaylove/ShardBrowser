# Changelog

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
