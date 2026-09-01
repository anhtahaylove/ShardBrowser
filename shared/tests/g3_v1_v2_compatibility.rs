//! G3 — migration/compatibility gate: v0.1.28 artifacts survive v0.2.0.
//!
//! The v0.2.0 release adds an encrypted container format alongside the v1
//! portable snapshot. The compatibility claim being tested is narrow and
//! concrete:
//!
//! 1. A v1 snapshot produced by `snapshot::pack` is still openable by
//!    `snapshot::unpack` after the v2 modules landed — the v1 path is not
//!    disturbed by the new code sharing the crate.
//! 2. The v2 container is a *wrapper*, not a replacement: sealing a v1
//!    snapshot and opening it again yields the original v1 bytes, which then
//!    still unpack through the untouched v1 reader.
//! 3. v1 readers and v2 readers cannot be confused for one another — a v2
//!    container is not accepted as a v1 snapshot.
//!
//! These run against a real temporary user-data dir rather than fixtures, so
//! a regression in the v1 tar/gzip path fails here even if unit tests pass.

use std::fs;
use std::path::{Path, PathBuf};

use shardx_core::backup::{self, BackupParams, BackupSecrets, MAGIC};
use shardx_core::envelope::IntentIds;
use shardx_core::keys::{signing_key_id, DEK_LEN, STREAM_NONCE_PREFIX_LEN, WRAP_NONCE_LEN};
use shardx_core::signing::Ed25519SigningKey as SigningKey;
use shardx_core::snapshot;

/// Deterministic fixture bytes, matching the derivation used by the v2 unit
/// tests: `SHA-256("shardx-g2-fixture\0" || label)`, truncated.
fn fixture_bytes<const N: usize>(label: &str) -> [u8; N] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"shardx-g2-fixture\0");
    h.update(label.as_bytes());
    let d = h.finalize();
    let mut out = [0u8; N];
    let mut i = 0;
    let mut ctr = 0u8;
    while i < N {
        let take = (N - i).min(32);
        if ctr == 0 {
            out[i..i + take].copy_from_slice(&d[..take]);
        } else {
            let mut h2 = Sha256::new();
            h2.update(d);
            h2.update([ctr]);
            let d2 = h2.finalize();
            out[i..i + take].copy_from_slice(&d2[..take]);
        }
        i += take;
        ctr += 1;
    }
    out
}

/// The nine-field intent identity, built from the same fixtures the v2 unit
/// tests use so a mismatch here means a real API change, not a fixture drift.
fn intent_ids(signer: &SigningKey) -> IntentIds {
    IntentIds {
        snapshot_id: fixture_bytes("snapshot-1"),
        tenant_id: fixture_bytes("tenant-a"),
        fleet_id: fixture_bytes("fleet-a"),
        profile_id: fixture_bytes("profile-a"),
        lease_id: fixture_bytes("lease-1"),
        manifest_replay_id: fixture_bytes("manifest-replay-1"),
        server_instance_id: fixture_bytes("server-instance-1"),
        fkek_key_id: shardx_core::keys::root_key_id(&fixture_bytes::<32>("fkek-gen-1")),
        intended_signer_signing_key_id: signing_key_id(&signer.verifying_key().to_bytes()),
    }
}

/// A self-cleaning temp dir. Avoids adding a dev-dependency for three tests.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "shardx-g3-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        TempDir(base)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Build a minimal but realistic user-data dir: nested profile directories and
/// files of a few different shapes, so the tar round-trip is actually exercised.
fn seed_udd(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let files: Vec<(PathBuf, Vec<u8>)> = vec![
        (root.join("Local State"), br#"{"os_crypt":{}}"#.to_vec()),
        (
            root.join("Default").join("Preferences"),
            br#"{"profile":{"name":"g3"}}"#.to_vec(),
        ),
        (
            root.join("Default").join("Secure Preferences"),
            br#"{"protection":{}}"#.to_vec(),
        ),
        (
            root.join("Default").join("Extensions").join("marker.bin"),
            (0u8..=255).cycle().take(4096).collect(),
        ),
    ];
    for (p, body) in &files {
        fs::create_dir_all(p.parent().unwrap()).expect("create parent");
        fs::write(p, body).expect("write seed file");
    }
    files
}

fn assert_seed_present(root: &Path, files: &[(PathBuf, Vec<u8>)], original_root: &Path) {
    for (p, body) in files {
        let rel = p.strip_prefix(original_root).expect("seed under root");
        let restored = root.join(rel);
        let got = fs::read(&restored)
            .unwrap_or_else(|e| panic!("restored file missing: {} ({e})", restored.display()));

        // `Local State` is deliberately NOT byte-identical: the v1 restore path
        // rewrites os_crypt's `encrypted_key` so the profile is decryptable by
        // *this* machine's DPAPI/keyring. Requiring equality here would assert
        // the opposite of the intended portability behaviour, so only the
        // structural invariant is checked.
        if rel == Path::new("Local State") {
            let text = String::from_utf8(got).expect("Local State is UTF-8");
            assert!(
                text.contains("\"os_crypt\""),
                "restored Local State lost its os_crypt block"
            );
            continue;
        }

        assert_eq!(&got, body, "content mismatch for {}", rel.display());
    }
}

/// 1. The v1 path still works end-to-end after the v2 modules landed.
#[test]
fn v1_snapshot_still_round_trips_after_v2_landed() {
    let src = TempDir::new("v1src");
    let files = seed_udd(src.path());

    let packed = snapshot::pack(src.path()).expect("v1 pack");
    assert!(!packed.is_empty(), "v1 pack produced no bytes");

    let dst = TempDir::new("v1dst");
    let udd = dst.path().join("User Data");
    fs::create_dir_all(&udd).expect("create dst udd");
    snapshot::unpack(&packed, &udd).expect("v1 unpack");

    assert_seed_present(&udd, &files, src.path());
}

/// 2. v2 wraps v1 losslessly: seal(v1) -> open -> identical v1 bytes, which
///    still unpack through the v1 reader.
#[test]
fn v2_container_wraps_a_v1_snapshot_losslessly() {
    let src = TempDir::new("wrapsrc");
    let files = seed_udd(src.path());
    let packed = snapshot::pack(src.path()).expect("v1 pack");

    let fkek: [u8; 32] = fixture_bytes("fkek-gen-1");
    let dek: [u8; DEK_LEN] = fixture_bytes("dek-snapshot-1");
    let wrap_nonce: [u8; WRAP_NONCE_LEN] = fixture_bytes("wrap-nonce-1");
    let prefix: [u8; STREAM_NONCE_PREFIX_LEN] = fixture_bytes("stream-prefix-1");
    let ctx_nonce: [u8; 16] = fixture_bytes("envelope-context-nonce-1");
    let signer = SigningKey::from_bytes(&fixture_bytes::<32>("issuer-signing-key"));

    let params = BackupParams {
        ids: intent_ids(&signer),
        key_generation: 1,
        target_version: 1,
        base_version: 0,
        fencing_token: 1,
        restore_epoch: 1,
        created_at_ms: 1_756_000_000_000,
        envelope_context_nonce: &ctx_nonce,
        previous_signed_head_hash: None,
    };
    let secrets = BackupSecrets {
        fkek: &fkek,
        dek: &dek,
        wrap_nonce: &wrap_nonce,
        stream_nonce_prefix: &prefix,
    };

    let mut sealed = Vec::new();
    let outcome = backup::seal(
        &mut packed.as_slice(),
        &mut sealed,
        &params,
        &secrets,
        &signer,
    )
    .expect("v2 seal");
    assert_eq!(outcome.plaintext_len, packed.len() as u64);
    assert!(
        sealed.len() > packed.len(),
        "container must carry framing overhead"
    );

    let signer_key_id = shardx_core::keys::signing_key_id(&signer.verifying_key().to_bytes());
    let mut recovered = Vec::new();
    backup::open(
        &mut sealed.as_slice(),
        &mut recovered,
        &fkek,
        &signer_key_id,
    )
    .expect("v2 open");

    assert_eq!(
        recovered, packed,
        "v2 must return the exact v1 bytes it was given"
    );

    // And those recovered bytes still satisfy the untouched v1 reader.
    let dst = TempDir::new("wrapdst");
    let udd = dst.path().join("User Data");
    fs::create_dir_all(&udd).expect("create dst udd");
    snapshot::unpack(&recovered, &udd).expect("v1 unpack of v2-recovered bytes");
    assert_seed_present(&udd, &files, src.path());
}

/// 3. The two formats are mutually distinguishable: a v2 container must not be
///    silently accepted by the v1 reader.
#[test]
fn v1_reader_rejects_a_v2_container() {
    let src = TempDir::new("mixsrc");
    seed_udd(src.path());
    let packed = snapshot::pack(src.path()).expect("v1 pack");

    let fkek: [u8; 32] = fixture_bytes("fkek-gen-1");
    let ctx_nonce: [u8; 16] = fixture_bytes("envelope-context-nonce-1");
    let dek: [u8; DEK_LEN] = fixture_bytes("dek-snapshot-1");
    let wrap_nonce: [u8; WRAP_NONCE_LEN] = fixture_bytes("wrap-nonce-1");
    let prefix: [u8; STREAM_NONCE_PREFIX_LEN] = fixture_bytes("stream-prefix-1");
    let signer = SigningKey::from_bytes(&fixture_bytes::<32>("issuer-signing-key"));

    let mut sealed = Vec::new();
    backup::seal(
        &mut packed.as_slice(),
        &mut sealed,
        &BackupParams {
            ids: intent_ids(&signer),
            key_generation: 1,
            target_version: 1,
            base_version: 0,
            fencing_token: 1,
            restore_epoch: 1,
            created_at_ms: 1_756_000_000_000,
            envelope_context_nonce: &ctx_nonce,
            previous_signed_head_hash: None,
        },
        &BackupSecrets {
            fkek: &fkek,
            dek: &dek,
            wrap_nonce: &wrap_nonce,
            stream_nonce_prefix: &prefix,
        },
        &signer,
    )
    .expect("v2 seal");

    // The container is self-identifying, and its magic is not gzip's.
    assert!(
        sealed.starts_with(MAGIC),
        "v2 container must carry its magic"
    );
    assert_ne!(&sealed[..2], &[0x1f, 0x8b], "v2 must not look like gzip");

    let dst = TempDir::new("mixdst");
    let udd = dst.path().join("User Data");
    fs::create_dir_all(&udd).expect("create dst udd");
    assert!(
        snapshot::unpack(&sealed, &udd).is_err(),
        "v1 reader must reject a v2 container rather than misparse it"
    );
}
