# Durable Design Consensus — ShardBrowser v0.2.x

**Status:** APPROVED FOR G2 SPIKE ONLY
**Production implementation:** BLOCKED pending independent verifier G2 PASS
**Production readiness:** BLOCKED pending G0–G7 and `P-OP`

## Persisted review order

1. Architect: APPROVE.
2. Critic: APPROVE after Architect.

## Canonical artifacts

- `.omx/plans/shardbrowser-v0.2.x-team-fleet-encrypted-backup.md`
- `.omx/plans/open-questions.md`
- `.omx/plans/architect-review-v6.md`
- `.omx/plans/critic-review-final.md`
- `docs/NEXT_V0.2_X_GOAL.md`

## Approved architecture

- Additive `/v2` control plane plus a separate local `team-sync.db`; v0.1.28 local profile JSON/user-data remains the local source of truth.
- Windows-first Team runtime; v0.2.0 internal foundation, v0.2.1 pilot sync, v0.2.2 recovery/rotation/hardening.
- Trusted coordinator and live coordination/RBAC SQLite control plane; artifact ciphertext/signed bytes remain client-verified and untrusted for confidentiality/artifact integrity.
- Tenant RBAC, explicit capabilities and root custody; separate signing and HPKE device keys.
- TRK → immutable FKEK generation → per-snapshot DEK; exact root/fleet grant records and exact streaming envelope/manifest/commit/receipt contracts.
- One current lease, monotonic fencing, exact idempotent stored responses, crash-safe resumable upload, strict restore, external restore-epoch authority and safe downgrade.
- Preserve the complete v0.1.28 96-tool MCP descriptor contract; new tools are additive only after API contracts stabilize.

## Remaining hard gates

- G2 dependency/security/durability spike with independent verifier readback.
- `P-OP` named production operator and disposable recovery/rollback drill.

No implementation, commit, tag, publish, release or runtime upgrade was authorized or performed by this design consensus.
