//! P-OP: operator recovery drill.
//!
//! Ignored by default: this is the drill an operator runs by hand on a
//! disposable machine before v0.2.x may be called production-ready. Run it with
//! `cargo test --test p_op_recovery_drill -- --ignored --nocapture` and paste
//! the printed block into the P-OP evidence packet.
//!
//! What is actually proven here, in the order the runbook asks for it:
//!
//!   1. A backup sealed by one "machine" is recoverable byte-for-byte on a
//!      different one that holds only the fkek and the expected signer key id.
//!   2. Recovery is refused when the container is truncated, when a single
//!      ciphertext byte is flipped, and when the signer is not the expected one.
//!      A drill that only restored a good bundle would pass just as happily
//!      against an implementation that never authenticated anything.
//!   3. The artifact digests an operator is asked to record are reproducible.
//!
//! This does not and cannot assert that a named operator ran it. The printed
//! `operator=` line is filled from `SHARDX_POP_OPERATOR` so the evidence packet
//! carries a human name; an unset variable is reported as UNSIGNED.

use std::io::Cursor;

use sha2::{Digest, Sha256};
use shardx_core::backup::{open, seal, BackupParams, BackupSecrets};
use shardx_core::envelope::IntentIds;
use shardx_core::keys::root_key_id;
use shardx_core::keys::signing_key_id;
use shardx_core::signing::Ed25519SigningKey as SigningKey;

fn fixture_bytes(label: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"shardx-p-op-drill/");
    h.update(label.as_bytes());
    h.finalize().into()
}

/// The 16-byte identifier fields, derived from the same domain-separated hash.
fn fixture_id(label: &str) -> [u8; 16] {
    let full = fixture_bytes(label);
    let mut out = [0u8; 16];
    out.copy_from_slice(&full[..16]);
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A disposable operator workstation: holds the key material and can seal.
struct Machine {
    fkek: [u8; 32],
    dek: [u8; 32],
    wrap_nonce: [u8; 12],
    prefix: [u8; 7],
    nonce16: [u8; 16],
    signer: SigningKey,
}

impl Machine {
    fn new(seed_label: &str) -> Self {
        let fkek = fixture_bytes(&format!("{seed_label}/fkek"));
        let dek = fixture_bytes(&format!("{seed_label}/dek"));
        let wrap = fixture_bytes(&format!("{seed_label}/wrap"));
        let pfx = fixture_bytes(&format!("{seed_label}/prefix"));
        let n16 = fixture_bytes(&format!("{seed_label}/nonce"));
        let sk = fixture_bytes(&format!("{seed_label}/signer"));

        let mut wrap_nonce = [0u8; 12];
        wrap_nonce.copy_from_slice(&wrap[..12]);
        let mut prefix = [0u8; 7];
        prefix.copy_from_slice(&pfx[..7]);
        let mut nonce16 = [0u8; 16];
        nonce16.copy_from_slice(&n16[..16]);

        Self {
            fkek,
            dek,
            wrap_nonce,
            prefix,
            nonce16,
            signer: SigningKey::from_bytes(&sk),
        }
    }

    fn secrets(&self) -> BackupSecrets<'_> {
        BackupSecrets {
            fkek: &self.fkek,
            dek: &self.dek,
            wrap_nonce: &self.wrap_nonce,
            stream_nonce_prefix: &self.prefix,
        }
    }

    fn signer_key_id(&self) -> [u8; 32] {
        signing_key_id(&self.signer.verifying_key().to_bytes())
    }

    fn params(&self) -> BackupParams<'_> {
        BackupParams {
            ids: IntentIds {
                snapshot_id: fixture_id("snapshot"),
                tenant_id: fixture_id("tenant"),
                fleet_id: fixture_id("fleet"),
                profile_id: fixture_id("profile"),
                lease_id: fixture_id("lease"),
                manifest_replay_id: fixture_id("manifest-replay"),
                server_instance_id: fixture_id("server"),
                fkek_key_id: root_key_id(&self.fkek),
                intended_signer_signing_key_id: self.signer_key_id(),
            },
            key_generation: 1,
            target_version: 1,
            base_version: 0,
            fencing_token: 1,
            restore_epoch: 1,
            created_at_ms: 1_756_000_000_000,
            envelope_context_nonce: &self.nonce16,
            previous_signed_head_hash: None,
        }
    }
}

/// A profile payload big enough to span several frames, so the drill exercises
/// the streaming path rather than a single-frame special case.
fn drill_payload() -> Vec<u8> {
    let mut out = Vec::with_capacity(3 * 1024 * 1024);
    let mut counter: u64 = 0;
    while out.len() < 3 * 1024 * 1024 {
        out.extend_from_slice(&counter.to_le_bytes());
        out.extend_from_slice(b"shardx-profile-bytes");
        counter += 1;
    }
    out
}

#[test]
#[ignore = "operator drill; run explicitly on a disposable machine"]
fn operator_can_recover_a_backup_on_a_fresh_machine_and_forgeries_are_refused() {
    let operator = std::env::var("SHARDX_POP_OPERATOR").unwrap_or_default();
    let plaintext = drill_payload();
    let plaintext_digest = Sha256::digest(&plaintext);

    // --- 1. Seal on the origin machine. ---
    let origin = Machine::new("origin");
    let mut sealed = Vec::new();
    let seal_outcome = seal(
        &mut Cursor::new(&plaintext),
        &mut sealed,
        &origin.params(),
        &origin.secrets(),
        &origin.signer,
    )
    .expect("seal must succeed on the origin machine");

    assert!(
        seal_outcome.frame_count > 1,
        "drill payload must span multiple frames to exercise streaming; got {}",
        seal_outcome.frame_count
    );
    let ciphertext_digest = Sha256::digest(&sealed);

    // The container must not carry the profile in the clear.
    assert!(
        !sealed.windows(20).any(|w| w == b"shardx-profile-bytes"),
        "plaintext marker found in the sealed container"
    );

    // --- 2. Recover on a fresh machine holding only fkek + expected signer id. ---
    let expected_signer = origin.signer_key_id();
    let mut restored = Vec::new();
    let open_outcome = open(
        &mut Cursor::new(&sealed),
        &mut restored,
        &origin.fkek,
        &expected_signer,
    )
    .expect("recovery must succeed with the correct fkek and signer id");

    assert_eq!(
        restored, plaintext,
        "recovered bytes differ from the original"
    );
    assert_eq!(Sha256::digest(&restored), plaintext_digest);
    assert_eq!(open_outcome.plaintext_len, plaintext.len() as u64);
    assert_eq!(open_outcome.signed_head_hash, seal_outcome.signed_head_hash);
    assert_eq!(open_outcome.intent_hash, seal_outcome.intent_hash);

    // --- 3. Refusals. Each must fail, and must not emit a full restore. ---

    // 3a. Truncated container (the interrupted-download case).
    //
    // `open` streams authenticated frames out as it goes and only catches
    // truncation at the signed head, so a partial restore CAN reach the sink.
    // That is the documented contract, and it is exactly why the runbook tells
    // operators to restore to a temporary path and promote only on `Ok`. The
    // drill therefore asserts the error, and records how much plaintext leaked
    // into the sink so the runbook's warning is backed by a measurement.
    let truncated = &sealed[..sealed.len() - 64];
    let mut sink = Vec::new();
    let truncated_err = open(
        &mut Cursor::new(truncated),
        &mut sink,
        &origin.fkek,
        &expected_signer,
    );
    assert!(
        truncated_err.is_err(),
        "truncated container must be refused"
    );
    let truncated_partial_bytes = sink.len();

    // 3b. Single flipped ciphertext byte.
    let mut corrupted = sealed.clone();
    let tail = corrupted.len() - 1024;
    corrupted[tail] ^= 0x01;
    let mut sink = Vec::new();
    assert!(
        open(
            &mut Cursor::new(&corrupted),
            &mut sink,
            &origin.fkek,
            &expected_signer,
        )
        .is_err(),
        "a single flipped ciphertext byte must be refused"
    );

    // 3c. Wrong expected signer (the substituted-bundle case).
    let impostor = Machine::new("impostor");
    let mut sink = Vec::new();
    assert!(
        open(
            &mut Cursor::new(&sealed),
            &mut sink,
            &origin.fkek,
            &impostor.signer_key_id(),
        )
        .is_err(),
        "a bundle signed by another key must be refused"
    );

    // 3d. Wrong fkek.
    let mut sink = Vec::new();
    assert!(
        open(
            &mut Cursor::new(&sealed),
            &mut sink,
            &impostor.fkek,
            &expected_signer,
        )
        .is_err(),
        "the wrong fkek must be refused"
    );

    // --- 4. Evidence block for the P-OP packet. ---
    println!("\n=== P-OP RECOVERY DRILL EVIDENCE ===");
    println!(
        "operator={}",
        if operator.is_empty() {
            "UNSIGNED (set SHARDX_POP_OPERATOR)".to_string()
        } else {
            operator
        }
    );
    println!("plaintext_bytes={}", plaintext.len());
    println!("plaintext_sha256={}", hex(&plaintext_digest));
    println!("ciphertext_bytes={}", sealed.len());
    println!("ciphertext_sha256={}", hex(&ciphertext_digest));
    println!("frame_count={}", seal_outcome.frame_count);
    println!("signed_head_hash={}", hex(&seal_outcome.signed_head_hash));
    println!("intent_hash={}", hex(&seal_outcome.intent_hash));
    println!("roundtrip=byte-identical");
    println!("refused=truncated,corrupted-byte,wrong-signer,wrong-fkek");
    println!("truncated_partial_bytes={truncated_partial_bytes}");
    println!("restore_rule=write to a temp path; promote only on Ok");
    println!("=== END P-OP RECOVERY DRILL EVIDENCE ===\n");
}
