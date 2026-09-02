# Architect review v4 — byte schemas and handoff order

Verdict: **REVISE**

No P0 remains. Close the two provider-independent byte-schema contracts and one
handoff-order contradiction before Critic review.

## Canonical authorization records

Define provider-independent canonical records with exact field name, type,
domain separator, record version, required/optional status, replay ID, validity,
and equality mapping to indexed columns:

- `DeviceApprovalV2`
- `TenantCapabilityGrantV2`
- `FleetKeyGrantV2::DeviceHpkeGrant`
- `FleetKeyGrantV2::RecoveryGrant`
- `FleetKeyGrantV2::RotationGrant`

Each signed container must include canonical payload bytes, payload hash,
signature suite/version, signature bytes, issuer key ID, and signed-container
hash. For HPKE grants, the signed payload must bind HPKE suite, recipient HPKE
key ID, encapsulated key bytes, wrapped fleet-key bytes, tenant/fleet/generation,
server instance, restore epoch, validity, and replay ID. Authorization/key
release verifies signature and exact payload-to-column equality before using any
index.

## Exact manifest, commit request, and receipt bytes

Define:

- `SignedSnapshotManifestV2`: canonical manifest payload bytes + payload hash +
  signature suite/version + signer key ID + signature bytes + exact container
  hash.
- `CommitRequestV2`: version/domain, tenant/fleet/profile, upload ID,
  idempotency key, canonical request hash, lease/fence/base version, intent hash,
  ciphertext hash/size, exact signed-manifest container bytes/hash, and client
  request nonce.
- `CommitReceiptBindingV2`: version/domain, exact request hash, snapshot ID,
  resulting version/head hash, lease release outcome, server instance/epoch,
  commit timestamp, and server receipt ID.

Local and server schema persist the exact signed-manifest container and exact
commit request/receipt binding bytes, not payload-only placeholders. Restart
replay must be byte-identical.

## Handoff order

After Architect and Critic approve this plan, a bounded research/dependency/
durability spike lane may execute G2. Production implementation does not begin
until G2 passes. The goal prompt must not give a production executor authority
to choose primitives before G2; it may coordinate the spike and then stop if G2
fails.

## Required validation

- canonical encode/decode round-trip and non-canonical rejection for every
  record;
- payload-to-column equality mismatch rejection, including HPKE fields;
- close/reopen byte-identical CommitRequest replay and receipt verification;
- goal/handoff wording consistency grep or structural check.
