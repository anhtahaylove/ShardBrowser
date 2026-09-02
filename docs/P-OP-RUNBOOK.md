# P-OP: production operator runbook and recovery drill

Status: **the `P-OP` gate is not passed.** Gates G0–G7 cover implementation
completeness; `P-OP` is separate and needs a *named* operator who has run the
drill below on a disposable machine, with a verifier confirming the output.
Until then every v0.2.x artifact stays labelled **non-production-ready**, and
the plan (§19, row `P-OP`) forbids production migration.

This document exists so that when an operator is named, the drill is a
checklist rather than an improvisation.

---

## 0. The one thing to read before restoring anything

**Restore to a temporary path. Promote only when the tool reports `Ok`.**

`backup::open` streams each authenticated frame straight to the output and can
only detect a truncated container when it reaches the signed head at the very
end. The drill measures this: truncating the last 64 bytes of a 3,145,744-byte
backup still emitted **3,145,744 bytes** — the complete profile — before the
call failed.

A restore written in place would therefore leave behind a profile that looks
complete and is not authenticated. The bytes are individually authenticated,
so this is not a forgery channel; it is a *completeness* channel, and the
mitigation is the promote-on-`Ok` rule, not a code change.

---

## 1. Prerequisites

- A disposable Windows machine. Do **not** use the canonical
  `VN Automation 001 - No Proxy` profile for any step here.
- A disposable server instance with its own `SHARDX_DATA_DIR`.
- The repository at the tag under test, and a working Rust toolchain.
- Never paste real key material, passphrases, tokens or proxy credentials into
  the evidence packet. Record any encountered secret as `[REDACTED]`.

Server configuration is environment-driven (`server/src/config.rs`). For a
disposable instance set at minimum:

| Variable | Why it matters for the drill |
| --- | --- |
| `SHARDX_BIND` | Keep `127.0.0.1:8080`. `0.0.0.0` exposes the disposable box. |
| `SHARDX_DATA_DIR` | Point at a scratch directory you can delete afterwards. |
| `SHARDX_TOKEN_SECRET` | Set explicitly; unset means an ephemeral secret and every restart invalidates tokens. |
| `SHARDX_ADMIN_PASS` | Must not stay `admin`; the server warns because that default is reachable by anyone who reaches the port. |

---

## 2. Fresh-server and Launcher-backup check

1. Start the disposable server with an empty `SHARDX_DATA_DIR`.
2. Confirm it applies migrations up to `0004_v2_team_fleet.sql` cleanly.
3. From the Launcher, take a backup of a **disposable** profile.
4. Record the artifact path and its SHA-256.

## 3. Safe downgrade / rollback check

1. Note the currently installed Launcher version.
2. Install the previous release over it.
3. Confirm the older Launcher starts, reads its own profile store, and does not
   consume or rewrite a v2 backup container. (`shared/tests/g3_v1_v2_compatibility.rs`
   covers the format side: a v1 reader must reject a v2 container.)
4. Re-install the version under test and confirm the profile still opens.

## 4. Recovery-bundle readback drill

Run on the disposable machine, from the repository root:

```
cd shared
SHARDX_POP_OPERATOR="<operator name>" cargo test --test p_op_recovery_drill -- --ignored --nocapture
```

The drill seals a multi-frame payload, recovers it on a second simulated
machine holding only the fkek and the expected signer key id, and then confirms
that four separate forgeries are refused:

| Case | Expectation |
| --- | --- |
| Truncated container | refused (see §0 for the partial-output caveat) |
| One flipped ciphertext byte | refused |
| Bundle signed by another key | refused |
| Wrong fkek | refused |

It also asserts the sealed container carries no plaintext profile marker, and
that the recovered bytes are identical to the original.

Copy the printed `=== P-OP RECOVERY DRILL EVIDENCE ===` block into the evidence
packet. An unset `SHARDX_POP_OPERATOR` prints `UNSIGNED`, which does **not**
satisfy the gate.

## 5. Artifact SHA-256

Record the digest of every artifact the operator handled:

```
sha256sum <installer> <backup-container>
```

Digests must match what the release publishes. A mismatch fails the gate.

---

## 6. Sign-off

The gate needs all four rows filled in, with a verifier who is not the
operator:

| Item | Evidence | Operator | Verifier |
| --- | --- | --- | --- |
| Fresh server + Launcher backup | | | |
| Safe downgrade / rollback | | | |
| Recovery-bundle readback | drill evidence block | | |
| Artifact SHA-256 | digest list | | |

Until this table is complete, the correct label remains
**non-production-ready**, and no production migration may proceed.
