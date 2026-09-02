//! Wire format for the v2 fleet snapshot manifest.
//!
//! This lives in `shared` on purpose. The manifest is a contract between the
//! Launcher (which signs) and the team server (which verifies), and when each
//! side keeps its own copy of the field list they drift: the client builds
//! something the server rejects, and nothing catches it until a real sync
//! fails. One definition, used by both sides and by the end-to-end test.

use crate::canonical as c;
use crate::signing::{build_signed_container, identity_key_id, Ed25519SigningKey};

/// Authorization domain. Must match the server constant exactly; a record
/// signed under another domain is refused even with a valid signature.
pub const DOMAIN_SNAPSHOT_MANIFEST: &str = "shardx.authorization.snapshot-manifest.v2";

/// Everything the manifest binds to.
#[derive(Debug, Clone)]
pub struct ManifestFields {
    pub tenant_id: [u8; 16],
    pub server_instance_id: [u8; 16],
    pub restore_epoch: u64,
    pub replay_id: [u8; 16],
    pub profile_id: [u8; 16],
    pub snapshot_id: [u8; 16],
    pub fleet_id: [u8; 16],
    pub base_version: u64,
    pub key_generation: u64,
    pub container_sha256: [u8; 32],
    pub not_before_ms: u64,
    pub not_after_ms: u64,
}

/// Build the signed manifest bytes the server expects.
///
/// The canonical encoder fixes the byte-level encoding, so the bytes signed
/// here are the bytes the verifier hashes.
pub fn build_snapshot_manifest(signer: &Ed25519SigningKey, f: &ManifestFields) -> Vec<u8> {
    let fields = vec![
        ("container_domain", c::t(DOMAIN_SNAPSHOT_MANIFEST)),
        ("container_version", c::u(1)),
        ("tenant_id", c::b(&f.tenant_id)),
        ("server_instance_id", c::b(&f.server_instance_id)),
        ("restore_epoch", c::u(f.restore_epoch)),
        ("replay_id", c::b(&f.replay_id)),
        ("not_before_ms", c::u(f.not_before_ms)),
        ("not_after_ms", c::u(f.not_after_ms)),
        ("profile_id", c::b(&f.profile_id)),
        ("snapshot_id", c::b(&f.snapshot_id)),
        ("fleet_id", c::b(&f.fleet_id)),
        ("base_version", c::u(f.base_version)),
        ("key_generation", c::u(f.key_generation)),
        ("container_sha256", c::b(&f.container_sha256)),
        (
            "issuer_signing_key_id",
            c::b(&identity_key_id(&signer.verifying_key())),
        ),
    ];
    build_signed_container(signer, fields).exact_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> ManifestFields {
        ManifestFields {
            tenant_id: [1u8; 16],
            server_instance_id: [2u8; 16],
            restore_epoch: 0,
            replay_id: [3u8; 16],
            profile_id: [4u8; 16],
            snapshot_id: [5u8; 16],
            fleet_id: [6u8; 16],
            base_version: 0,
            key_generation: 1,
            container_sha256: [7u8; 32],
            not_before_ms: 1_000,
            not_after_ms: 2_000,
        }
    }

    /// Signing the same fields twice must produce identical bytes, otherwise
    /// the signature could not be checked against a re-encoding.
    #[test]
    fn encoding_is_deterministic() {
        let sk = Ed25519SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(
            build_snapshot_manifest(&sk, &fields()),
            build_snapshot_manifest(&sk, &fields())
        );
    }

    /// A changed field must change the bytes: this is what binds a manifest to
    /// one specific container.
    #[test]
    fn a_different_container_changes_the_manifest() {
        let sk = Ed25519SigningKey::from_bytes(&[9u8; 32]);
        let mut other = fields();
        other.container_sha256 = [8u8; 32];
        assert_ne!(
            build_snapshot_manifest(&sk, &fields()),
            build_snapshot_manifest(&sk, &other)
        );
    }
}
