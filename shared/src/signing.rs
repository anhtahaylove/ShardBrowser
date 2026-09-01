//! Ed25519 signing identity and signed-container assembly for ShardX v0.2.
//!
//! Two invariants carry the security of every authorization, manifest and
//! transition record:
//!
//!   * **Domain-separated signing input.** A signature is never computed over
//!     bare canonical bytes. The input is
//!     `"SHARDX-SIGNED-RECORD-V2\0" + u32be(len(TBS)) + TBS`, so a payload
//!     signed as one record type can never be replayed as another.
//!   * **Acyclic commitment order.** The to-be-signed bytes cover the record
//!     fields only. `signature_bytes` is added afterwards to form the container
//!     core, and `signed_container_hash` is computed over that core — so
//!     nothing ever commits to a hash of itself.
//!
//! Verification is strict: exact 64-byte signatures, exact 32-byte public keys,
//! and non-canonical key encodings are rejected rather than coerced.
//!
//! Values here were validated byte-for-byte by the G2 conformance gate; the
//! golden vectors in the test module are pinned from that gate's report.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

// The caller needs these types to hold a signing identity and to check one, so
// they are part of this module's public surface rather than an internal detail.
pub use ed25519_dalek::{SigningKey as Ed25519SigningKey, VerifyingKey as Ed25519VerifyingKey};

use crate::canonical::{self as c, Value};
use crate::keys::signing_key_id;

/// `signature_suite_id` — Ed25519 (RFC 8032) with strict parsing.
pub const SIGNATURE_SUITE_ID_ED25519: u16 = 1;
/// `signature_version` for the pinned suite profile.
pub const SIGNATURE_VERSION: u16 = 1;
/// Exact Ed25519 signature length, within the `signature_bytes` bound (1, 4096).
pub const ED25519_SIG_LEN: usize = 64;

#[derive(Debug, PartialEq, Eq)]
pub enum SignError {
    /// Signature did not verify. Carries no detail by design.
    Signature,
    BadLength {
        field: &'static str,
        got: usize,
    },
    NonCanonicalKeyEncoding,
}

impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Signature => write!(f, "signature verification failed"),
            Self::BadLength { field, got } => write!(f, "invalid length for {field}: got {got}"),
            Self::NonCanonicalKeyEncoding => write!(f, "non-canonical public key encoding"),
        }
    }
}

impl std::error::Error for SignError {}

// ------------------------------------------------------- signing contract ---

/// `authorization_tbs_bytes` signature input:
/// ASCII `SHARDX-SIGNED-RECORD-V2\0` + u32be(len(TBS)) + exact TBS bytes.
///
/// The length prefix makes the preimage unambiguous; the label prevents a
/// signature from being valid under a different record domain.
pub fn signed_record_signing_input(tbs_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(c::L_SIGNED_RECORD.len() + 4 + tbs_bytes.len());
    out.extend_from_slice(c::L_SIGNED_RECORD.as_bytes());
    out.extend_from_slice(&(tbs_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(tbs_bytes);
    out
}

pub fn sign_tbs(sk: &SigningKey, tbs_bytes: &[u8]) -> [u8; ED25519_SIG_LEN] {
    sk.sign(&signed_record_signing_input(tbs_bytes)).to_bytes()
}

/// Strict verification: exact 64-byte signature, exact 32-byte public key.
pub fn verify_tbs(vk_bytes: &[u8], tbs_bytes: &[u8], sig_bytes: &[u8]) -> Result<(), SignError> {
    let vk_arr: [u8; 32] = vk_bytes.try_into().map_err(|_| SignError::BadLength {
        field: "verifying_key",
        got: vk_bytes.len(),
    })?;
    let sig_arr: [u8; ED25519_SIG_LEN] =
        sig_bytes.try_into().map_err(|_| SignError::BadLength {
            field: "signature_bytes",
            got: sig_bytes.len(),
        })?;
    let vk = VerifyingKey::from_bytes(&vk_arr).map_err(|_| SignError::NonCanonicalKeyEncoding)?;
    let sig = Signature::from_bytes(&sig_arr);
    vk.verify(&signed_record_signing_input(tbs_bytes), &sig)
        .map_err(|_| SignError::Signature)
}

/// Key ID of a signing identity, as embedded in `issuer_signing_key_id`.
pub fn identity_key_id(vk: &VerifyingKey) -> [u8; 32] {
    signing_key_id(vk.as_bytes())
}

// ---------------------------------------------- signed container assembly ---

/// A fully assembled signed container, exposing every byte string a caller may
/// legitimately need to persist, hash or transmit.
pub struct SignedContainer {
    /// Exact to-be-signed bytes (record fields only).
    pub tbs_bytes: Vec<u8>,
    /// TBS fields plus `signature_bytes`.
    pub core_bytes: Vec<u8>,
    /// `domain_hash(SHARDX-SIGNED-CONTAINER-V2, core_bytes)`.
    pub signed_container_hash: [u8; 32],
    /// Core fields plus `signed_container_hash` — the on-wire container.
    pub exact_bytes: Vec<u8>,
    pub exact_bytes_sha256: [u8; 32],
    pub signature_bytes: Vec<u8>,
}

/// Builds `SignedAuthorizationRecordV2`-shaped containers (also used for the
/// manifest and transition containers, which share the exact construction with
/// different field names).
///
/// `tbs_fields` are the record fields from `container_domain` through
/// `issuer_signing_key_id` — never `signature_bytes` or `signed_container_hash`,
/// which this function appends in the fixed order that keeps commitment acyclic.
pub fn build_signed_container(sk: &SigningKey, tbs_fields: Vec<(&str, Value)>) -> SignedContainer {
    let tbs_bytes = c::encode(&c::m(tbs_fields.clone()));

    let signature = sign_tbs(sk, &tbs_bytes);

    let mut core_fields = tbs_fields;
    core_fields.push(("signature_bytes", c::b(&signature)));
    let core_bytes = c::encode(&c::m(core_fields.clone()));

    let signed_container_hash = c::domain_hash(c::L_SIGNED_CONTAINER, &core_bytes);

    let mut outer_fields = core_fields;
    outer_fields.push(("signed_container_hash", c::b(&signed_container_hash)));
    let exact_bytes = c::encode(&c::m(outer_fields));
    let exact_bytes_sha256 = c::sha256(&exact_bytes);

    SignedContainer {
        tbs_bytes,
        core_bytes,
        signed_container_hash,
        exact_bytes,
        exact_bytes_sha256,
        signature_bytes: signature.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex32(h: &[u8; 32]) -> String {
        h.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn fixture_bytes<const N: usize>(label: &str) -> [u8; N] {
        use sha2::{Digest, Sha256};
        let mut out = [0u8; N];
        let mut counter: u32 = 0;
        let mut filled = 0;
        while filled < N {
            let mut h = Sha256::new();
            h.update(b"SHARDX-G2-SPIKE-FIXTURE\0");
            h.update(label.as_bytes());
            h.update(counter.to_be_bytes());
            let block: [u8; 32] = h.finalize().into();
            let take = (N - filled).min(32);
            out[filled..filled + take].copy_from_slice(&block[..take]);
            filled += take;
            counter += 1;
        }
        out
    }

    fn keypair(seed_label: &str) -> SigningKey {
        SigningKey::from_bytes(&fixture_bytes::<32>(seed_label))
    }

    #[test]
    fn suite_constants_are_pinned() {
        assert_eq!(SIGNATURE_SUITE_ID_ED25519, 1);
        assert_eq!(SIGNATURE_VERSION, 1);
        assert_eq!(ED25519_SIG_LEN, 64);
    }

    #[test]
    fn signing_input_is_domain_and_length_prefixed() {
        let input = signed_record_signing_input(b"abc");
        let mut expected = Vec::new();
        expected.extend_from_slice(b"SHARDX-SIGNED-RECORD-V2\0");
        expected.extend_from_slice(&3u32.to_be_bytes());
        expected.extend_from_slice(b"abc");
        assert_eq!(input, expected);

        // Different splits of the same concatenation must not collide.
        assert_ne!(
            signed_record_signing_input(b"ab"),
            signed_record_signing_input(b"abc")
        );
    }

    #[test]
    fn sign_verify_roundtrips() {
        let sk = keypair("snapshot-signer");
        let vk = sk.verifying_key();
        let tbs = b"exact canonical tbs bytes";
        let sig = sign_tbs(&sk, tbs);
        assert_eq!(sig.len(), ED25519_SIG_LEN);
        assert_eq!(verify_tbs(vk.as_bytes(), tbs, &sig), Ok(()));
    }

    #[test]
    fn verify_rejects_mutated_tbs() {
        let sk = keypair("snapshot-signer");
        let vk = sk.verifying_key();
        let sig = sign_tbs(&sk, b"original tbs");
        assert_eq!(
            verify_tbs(vk.as_bytes(), b"modified tbs", &sig),
            Err(SignError::Signature)
        );
    }

    #[test]
    fn verify_rejects_mutated_signature_bytes() {
        let sk = keypair("snapshot-signer");
        let vk = sk.verifying_key();
        let tbs = b"exact canonical tbs bytes";
        let mut sig = sign_tbs(&sk, tbs);
        // Flip a bit in the signature itself, not the payload.
        sig[0] ^= 0x01;
        assert_eq!(
            verify_tbs(vk.as_bytes(), tbs, &sig),
            Err(SignError::Signature)
        );

        let mut sig2 = sign_tbs(&sk, tbs);
        let last = sig2.len() - 1;
        sig2[last] ^= 0x80;
        assert_eq!(
            verify_tbs(vk.as_bytes(), tbs, &sig2),
            Err(SignError::Signature)
        );
    }

    #[test]
    fn verify_rejects_wrong_signer() {
        let sk = keypair("snapshot-signer");
        let other = keypair("other-signer");
        let tbs = b"exact canonical tbs bytes";
        let sig = sign_tbs(&sk, tbs);
        assert_eq!(
            verify_tbs(other.verifying_key().as_bytes(), tbs, &sig),
            Err(SignError::Signature)
        );
    }

    #[test]
    fn verify_rejects_bad_lengths() {
        let sk = keypair("snapshot-signer");
        let vk = sk.verifying_key();
        let tbs = b"exact canonical tbs bytes";
        let sig = sign_tbs(&sk, tbs);

        assert_eq!(
            verify_tbs(&vk.as_bytes()[..31], tbs, &sig),
            Err(SignError::BadLength {
                field: "verifying_key",
                got: 31
            })
        );
        assert_eq!(
            verify_tbs(vk.as_bytes(), tbs, &sig[..63]),
            Err(SignError::BadLength {
                field: "signature_bytes",
                got: 63
            })
        );
    }

    #[test]
    fn signing_input_domain_differs_from_container_domain() {
        // A record signature must not be valid over a container preimage.
        assert_ne!(c::L_SIGNED_RECORD, c::L_SIGNED_CONTAINER);
    }

    #[test]
    fn container_commitment_is_acyclic() {
        let sk = keypair("snapshot-signer");
        let container = build_signed_container(
            &sk,
            vec![
                ("container_domain", c::t("shardx.auth.record.v2")),
                ("version", c::u(2)),
            ],
        );

        // TBS must not contain the signature, and the core must not contain the
        // container hash — otherwise a field would commit to a hash of itself.
        assert!(!container
            .tbs_bytes
            .windows(container.signature_bytes.len())
            .any(|w| w == container.signature_bytes.as_slice()));
        assert!(!container
            .core_bytes
            .windows(32)
            .any(|w| w == container.signed_container_hash));
        // The outer bytes do carry both.
        assert!(container
            .exact_bytes
            .windows(32)
            .any(|w| w == container.signed_container_hash));

        // And the signature actually verifies over the TBS bytes.
        assert_eq!(
            verify_tbs(
                sk.verifying_key().as_bytes(),
                &container.tbs_bytes,
                &container.signature_bytes
            ),
            Ok(())
        );
    }

    /// Pinned from the G2 report, probes `G2-VEC-auth-payload`,
    /// `G2-VEC-signed-container-hash` and `G2-VEC-signed-container`.
    /// Reproduces the gate's `SignedAuthorizationRecordV2` exactly: the nine
    /// pre-signature fields, in the plan's field set. Any change to the field
    /// set or the assembly sequence changes these hashes.
    #[test]
    fn g2_golden_vector_signed_container_hash() {
        let sk = keypair("issuer-signing-key");
        let vk = sk.verifying_key();

        let payload_bytes = c::encode(&c::m(vec![
            ("domain", c::t("shardx.authorization.device-enrollment.v2")),
            ("version", c::u(2)),
            ("tenant_id", c::b(&fixture_bytes::<16>("tenant-a"))),
            ("device_id", c::b(&fixture_bytes::<16>("device-a"))),
            ("server_instance_id", c::b(&fixture_bytes::<16>("server-1"))),
            ("restore_epoch", c::u(7)),
        ]));
        let payload_sha256 = c::sha256(&payload_bytes);
        assert_eq!(
            hex32(&payload_sha256),
            "44fe61fe61e434009456550622718c9dd3e4f8a40128543d49b28effc7191412",
            "authorization payload diverged from G2-pinned vector"
        );

        let container = build_signed_container(
            &sk,
            vec![
                (
                    "container_domain",
                    c::t("shardx.authorization.signed-container.v2"),
                ),
                ("container_version", c::u(2)),
                (
                    "payload_domain",
                    c::t("shardx.authorization.device-enrollment.v2"),
                ),
                ("payload_version", c::u(2)),
                ("canonical_payload_bytes", c::b(&payload_bytes)),
                ("payload_sha256", c::b(&payload_sha256)),
                (
                    "signature_suite_id",
                    c::u(SIGNATURE_SUITE_ID_ED25519 as u64),
                ),
                ("signature_version", c::u(SIGNATURE_VERSION as u64)),
                ("issuer_signing_key_id", c::b(&identity_key_id(&vk))),
            ],
        );

        assert_eq!(
            hex32(&container.signed_container_hash),
            "93c5409668ec3a58151913f7e236855d9c9524cf75a4d52b9e0dc999b6cb5500",
            "signed_container_hash diverged from the G2-pinned golden vector"
        );
        assert_eq!(
            hex32(&container.exact_bytes_sha256),
            "78aa38d3c6fe96592c69d43aa787409f6a836b950e8b991a62265157c5d97dbc",
            "exact container bytes diverged from the G2-pinned golden vector"
        );
    }
}
