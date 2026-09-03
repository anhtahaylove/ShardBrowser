# Key custody and recovery

How profile encryption keys are held today, why fleet sync currently needs a
shared passphrase, and what the frozen v2 design says should replace it.

Written against `main` at the commit that added profile sync. Every claim here
was checked against the code, not the plan: where the plan describes something
that does not exist yet, this document says so.

## What exists today

### Local backup

`shared/src/backup_file.rs` derives the file key from a passphrase:

```
FKEK = Argon2id(passphrase, salt)     salt: 16 random bytes, stored in the file
DEK  = random 32 bytes                wrapped under FKEK
```

The DEK encrypts the profile; the FKEK only wraps the DEK. The salt travels in
the file header, so any machine can open a backup given the passphrase.

**There is no recovery path.** The passphrase is not stored, not escrowed, and
not derivable. A forgotten passphrase means the backup is unreadable — stated
plainly in `backup_cmd.rs` and in the UI before the user commits to it. This is
a deliberate trade, not an oversight: escrow that the Launcher could use
unattended is escrow an attacker with the same machine could also use.

### Fleet sync

Profile sync (`src-tauri/src/sync_cmd.rs`) uploads exactly the container
`backup_file` produces. The team server stores ciphertext, a SHA-256 and a
version; it holds no key and cannot open what it stores.

The consequence is honest but limited: **the passphrase is a shared secret
between devices.** Whoever pulls a profile must know the passphrase used to
push it, communicated out of band. There is no per-device key wrapping, so:

- Revoking one device's access means changing the passphrase and re-pushing.
- A leaked passphrase compromises every snapshot sealed under it.
- Nothing binds a container to the tenant that stored it.

That is the current state. It is usable for a small team that already shares
credentials, and it is not what the v2 design calls for.

### Device keys

Enrollment (`server/src/enrollment.rs`) registers two keys per device:

| Key | Purpose | Stored |
| --- | --- | --- |
| Ed25519 signing | Proof of possession; signs snapshot manifests | `team.json`, hex seed |
| X25519 HPKE recipient | Receives wrapped keys — **unused today** | `team.json`, hex seed |

The HPKE key is registered and never used. It exists because the design below
needs it, and enrollment is the only moment a device can introduce a key.

`team.json` sits beside the profile store in plaintext, protected by filesystem
permissions alone. For a prototype whose profiles are already on that disk this
adds no meaningful exposure, but it is not production key storage: an OS
credential store (DPAPI, Keychain, kernel keyring) is the intended home and is
not wired up.

## What the design calls for

`.omx/plans/…-team-fleet-encrypted-backup.md` §5.6.4 defines a tenant root key
(TRK) that never leaves a device in the clear:

```
TRK ──HPKE seal to device HPKE public key──▶ v2_tenant_root_key_grants
```

Each device gets its own sealed copy. `shared/src/grants.rs` implements the
sealing (`seal_trk`, `open_trk`, 32-byte encapsulation, 48-byte wrapped key,
AAD bound to the grant fields) and the schema in `0004_v2_team_fleet.sql` has
every column needed to store one.

The lifecycle is three grant variants:

- `FirstRootSelfGrant` — the bootstrap device creates the TRK and seals it to
  itself, once per tenant, inside one `BEGIN IMMEDIATE` transaction.
- `ExistingRootGrant` — an active custodian seals the current TRK to a newly
  enrolled device.
- `RotationGrant` — a custodian seals a new generation, after which the old one
  is retired.

Revocation is deliberately narrow: revoking a grant blocks *future* unwraps but
cannot retract a TRK a device already copied. Excluding a device means rotating
to a new generation and re-wrapping — the design says this explicitly, and it is
the right model to keep.

### What is missing

| Piece | State |
| --- | --- |
| `seal_trk` / `open_trk` | Implemented in `shared`; exercised end to end by the grant tests |
| `v2_tenant_root_key_grants` table | Stores filed grants |
| `POST /v2/tenant-root-key-grants` | Verifies, claims the replay id, and **stores** the sealed grant |
| `GET /v2/tenants/:tenant/devices/:device/root-key-grants` | Returns a device's sealed grants |
| Root generation lifecycle | No table, no endpoints |
| Grant issuance at enrollment | Not implemented — grants must be filed by an external custodian |
| Recovery bundle | Not implemented |

Storage and collection now exist: a grant sealed to a device's HPKE key can be
filed, read back, and opened by that device, and `v2_e2e.rs` proves the round
trip by recovering the key from what the server returned. The server stores
ciphertext it cannot read.

What is still missing is everything that would *produce* those grants
automatically. Nothing generates a TRK, nothing seals it to a newly enrolled
device, and no generation lifecycle governs rotation. So sync still falls back
to a shared passphrase: the transport for a root key exists, the ceremony that
would use it does not.

### A failure worth remembering

`record_table` in `v2_replay_ledger` had a CHECK constraint listing only the
device-approval and capability-grant tables. Root key grants claim their replay
id from the same ledger, so every attempt violated it.

The claim runs as `INSERT OR IGNORE ... changes()`, and OR IGNORE swallows a
CHECK violation exactly like a duplicate primary key — zero rows affected. The
endpoint read that as "already used" and answered `409`, so filing any grant
looked like a replay of a record that had never been stored. Grants were both
impossible to file and entirely unprotected against real replay.

Migration `0005` widens the constraint, and `consume_replay_id` now confirms the
row actually exists before reporting a replay, so a future constraint gap
surfaces as an error instead of a false accusation.

## Recovery

The design's recovery path is a *recovery bundle*: the TRK wrapped under a
root-held key, produced at bootstrap, verified by readback before a generation
activates. It is not implemented.

Until it is, recovery is exactly one thing: **someone remembers the
passphrase.** Losing it loses the snapshots sealed under it. Backups of
`team.json` preserve device identity — they do not preserve profile contents,
because the device key never encrypted them.

Operators should therefore treat the sync passphrase as a credential to be
recorded in whatever the team already uses for shared secrets, and should not
assume the server can help. It cannot; it never sees a key.

## Ordering the remaining work

Roughly the sequence the constraints force:

1. ~~**Persist TRK grants.**~~ Done: grants are stored and collectable, and a
   device can open its own grant. Issuance is still manual.
2. **Root generation lifecycle.** Table plus create/activate/revoke, with the
   `FirstRootSelfGrant` uniqueness constraint the plan specifies.
3. **Grant TRK at enrollment.** A custodian device seals the TRK to the new
   device's HPKE key — the key enrollment already registers.
4. **Seal containers under the TRK.** Replace the passphrase-derived FKEK for
   fleet containers, keeping it for local `.shxbak` files, which have no tenant
   and should stay openable on a machine that has never enrolled.
5. **Recovery bundle plus readback**, before any of this is called
   production-ready.
6. **Move device keys into the OS credential store.**

Steps 1–3 are prerequisites for 4; shipping 4 first would mean inventing a
distribution mechanism that the frozen design already specifies.

## Threat model, stated plainly

What holds today:

- The server cannot read profile contents. It stores sealed containers only.
- A stolen container is useless without the passphrase.
- Tenant boundaries are enforced on every `/v2/` route (`require_tenant_member`).
- A stale push is refused: version fencing stops one device silently
  overwriting another.

What does not hold:

- Per-device revocation. Everyone shares one passphrase.
- Forward secrecy across rotations. There are no rotations.
- Device key confidentiality against local disk access. `team.json` is plaintext.
- Recovery of anything, if the passphrase is lost.

`P-OP` remains blocked, and this is one of the reasons.
