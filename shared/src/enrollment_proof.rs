//! Wire format for v2 device enrollment.
//!
//! Same reasoning as `fleet_manifest`: the Launcher builds the proof and the
//! team server verifies it, so the field list must have exactly one
//! definition. Two copies drift silently — the client signs bytes the server
//! never reconstructs, and the only symptom is a rejected enrollment.

use crate::canonical as c;

/// Authorization domain for a device enrollment proof.
///
/// Distinct from every other domain so a signature collected here cannot be
/// replayed as a manifest, grant or approval.
pub const DOMAIN_DEVICE_ENROLLMENT_PROOF: &str = "shardx.authorization.device-enrollment-proof.v2";

/// Everything the proof commits to.
pub struct EnrollmentProofFields<'a> {
    pub challenge_id: &'a [u8; 16],
    pub nonce: &'a [u8; 32],
    pub tenant_id: &'a [u8; 16],
    pub account_id: &'a [u8; 16],
    pub signing_public_key: &'a [u8; 32],
    pub hpke_public_key: &'a [u8; 32],
}

/// Canonical bytes a device signs to prove it holds its signing key.
///
/// The nonce ties the signature to one server-issued challenge, and the keys
/// are inside the signed bytes so a proof cannot be lifted onto a different
/// key pair.
pub fn enrollment_proof_tbs(f: &EnrollmentProofFields<'_>) -> Vec<u8> {
    c::encode(&c::Value::Map(vec![
        (
            "container_domain".into(),
            c::t(DOMAIN_DEVICE_ENROLLMENT_PROOF),
        ),
        ("container_version".into(), c::u(1)),
        ("challenge_id".into(), c::b(f.challenge_id)),
        ("nonce".into(), c::b(f.nonce)),
        ("tenant_id".into(), c::b(f.tenant_id)),
        ("account_id".into(), c::b(f.account_id)),
        ("signing_public_key".into(), c::b(f.signing_public_key)),
        ("hpke_public_key".into(), c::b(f.hpke_public_key)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Changing any field must change the signed bytes; a field that does not
    /// affect the encoding is a field an attacker can alter freely.
    #[test]
    fn every_field_is_covered_by_the_signed_bytes() {
        let base = enrollment_proof_tbs(&EnrollmentProofFields {
            challenge_id: &[1u8; 16],
            nonce: &[2u8; 32],
            tenant_id: &[3u8; 16],
            account_id: &[4u8; 16],
            signing_public_key: &[5u8; 32],
            hpke_public_key: &[6u8; 32],
        });

        let variants = [
            enrollment_proof_tbs(&EnrollmentProofFields {
                challenge_id: &[9u8; 16],
                nonce: &[2u8; 32],
                tenant_id: &[3u8; 16],
                account_id: &[4u8; 16],
                signing_public_key: &[5u8; 32],
                hpke_public_key: &[6u8; 32],
            }),
            enrollment_proof_tbs(&EnrollmentProofFields {
                challenge_id: &[1u8; 16],
                nonce: &[9u8; 32],
                tenant_id: &[3u8; 16],
                account_id: &[4u8; 16],
                signing_public_key: &[5u8; 32],
                hpke_public_key: &[6u8; 32],
            }),
            enrollment_proof_tbs(&EnrollmentProofFields {
                challenge_id: &[1u8; 16],
                nonce: &[2u8; 32],
                tenant_id: &[9u8; 16],
                account_id: &[4u8; 16],
                signing_public_key: &[5u8; 32],
                hpke_public_key: &[6u8; 32],
            }),
            enrollment_proof_tbs(&EnrollmentProofFields {
                challenge_id: &[1u8; 16],
                nonce: &[2u8; 32],
                tenant_id: &[3u8; 16],
                account_id: &[9u8; 16],
                signing_public_key: &[5u8; 32],
                hpke_public_key: &[6u8; 32],
            }),
            enrollment_proof_tbs(&EnrollmentProofFields {
                challenge_id: &[1u8; 16],
                nonce: &[2u8; 32],
                tenant_id: &[3u8; 16],
                account_id: &[4u8; 16],
                signing_public_key: &[9u8; 32],
                hpke_public_key: &[6u8; 32],
            }),
            enrollment_proof_tbs(&EnrollmentProofFields {
                challenge_id: &[1u8; 16],
                nonce: &[2u8; 32],
                tenant_id: &[3u8; 16],
                account_id: &[4u8; 16],
                signing_public_key: &[5u8; 32],
                hpke_public_key: &[9u8; 32],
            }),
        ];

        for (i, v) in variants.iter().enumerate() {
            assert_ne!(&base, v, "field {i} does not affect the signed bytes");
        }
    }
}
