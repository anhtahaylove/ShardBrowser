# Rust SDK dependency audit for v0.1.28

Audited on 2026-08-14 with `cargo-audit 0.22.2` against the RustSec database
updated on 2026-08-12.

## RustSec result

- The stable Rust SDK lockfile contains 221 packages and reports zero
  vulnerabilities and zero informational warnings.
- The SDK enables `reqwest` with only `rustls-tls`, `json`, and `socks`; its
  optional HTTP/3 dependency graph is not active.
- A clean Rust 1.74-compatible resolution can contain an inactive
  `quinn-proto 0.11.12` entry because Cargo lockfiles include optional
  dependencies. RustSec reports two Quinn advisories for that lockfile, but
  `cargo tree -i quinn-proto` is empty for both supported SDK feature sets.
  No advisory is ignored or suppressed.

## Major dependency decisions

| Dependency | Before | Candidate reviewed | Decision |
| --- | --- | --- | --- |
| `chromiumoxide` | 0.7 | 0.8 / 0.9 | Keep 0.7. Both newer lines require Rust 1.85 and 0.8 changes the fetcher API. |
| `dirs` | 5 | 6.0.0 | Upgrade. Default-feature, no-default-feature, clippy, doctest, and Rust 1.74.1 checks pass. |
| `rand` | 0.8 | 0.9.5 / 0.10.2 | Upgrade to 0.9.5 and migrate to `IndexedRandom` plus `rand::rng()`. Keep 0.10 deferred because it requires Rust 1.85. |
| `reqwest` | 0.12.28 | 0.13.4 | Keep 0.12. The current line is RustSec-clean; 0.13.4 requires Rust 1.85 and renames the Rustls feature surface. |
| `zip` | 2.4.2 | 3.0.0 / 8.6.0 | Keep 2. The first newer major requires Rust 1.75 and the current stable major requires Rust 1.88. |

## Verification matrix

- Stable Rust: tests and doctests pass with all features and with no default
  features.
- Stable Rust: clippy passes with `-D warnings` for both feature sets.
- RustSec: the 221-package stable lockfile passes with no ignored advisories.
- Rust 1.74.1: tests and doctests pass for both feature sets using Cargo's
  incompatible-Rust-version fallback resolution.
- Packaging: the Rust SDK package builds with the same 221-package stable
  lockfile that passed RustSec.
