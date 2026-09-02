//! Round-trip tests for the local backup file format.
//!
//! These exercise the real `backup_file::{create, restore}` path against real
//! profile directories, not mocks: the point is to prove a profile survives
//! encryption and recovery byte-for-byte, and that a damaged or wrong-passphrase
//! file cannot silently produce a partial profile.

use std::fs;
use std::path::{Path, PathBuf};

use shardx_core::backup_file;

/// Disposable temp dir. Never touches a canonical profile.
fn scratch(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "shardx-backup-file-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).expect("create scratch");
    p
}

/// A small but non-trivial profile: nested dirs, a binary file, an empty file
/// and a non-ASCII name, so the round trip has to preserve structure and not
/// just one blob.
fn seed_profile(root: &Path) {
    fs::create_dir_all(root.join("Default/Local Storage")).unwrap();
    fs::write(
        root.join("Default/Preferences"),
        br#"{"profile":{"name":"t"}}"#,
    )
    .unwrap();
    fs::write(
        root.join("Default/Local Storage/leveldb.bin"),
        (0u8..=255).cycle().take(4096).collect::<Vec<u8>>(),
    )
    .unwrap();
    fs::write(root.join("Default/empty.txt"), b"").unwrap();
    fs::write(root.join("Default/tiếng-việt.txt"), "nội dung".as_bytes()).unwrap();
    fs::write(root.join("Local State"), br#"{"os_crypt":{}}"#).unwrap();
}

/// Every file under `root`, relative path -> bytes, for exact comparison.
fn snapshot_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap()).collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

const PASSPHRASE: &str = "a-long-enough-passphrase";

#[test]
fn a_profile_survives_backup_and_restore_byte_for_byte() {
    let base = scratch("roundtrip");
    let src = base.join("src");
    let dst = base.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    seed_profile(&src);
    let before = snapshot_tree(&src);

    let file = base.join("backup.shxbak");
    let info = backup_file::create("profile-1", &src, &file, PASSPHRASE).expect("create backup");

    assert!(file.exists(), "backup file was not written");
    assert!(info.file_bytes > 0);
    assert_eq!(info.sha256.len(), 64, "sha256 must be hex-encoded");
    assert!(
        info.plaintext_bytes > 0,
        "a seeded profile must pack to something"
    );

    backup_file::restore(&file, &dst, PASSPHRASE).expect("restore backup");
    let after = snapshot_tree(&dst);

    // Two paths are deliberately not byte-identical, and asserting equality on
    // them would assert against the design:
    //   - `Local State` is machine-bound and excluded, so the destination mints
    //     its own os_crypt key.
    //   - the `Cookies` DB is re-encrypted with the destination key, so `unpack`
    //     rebuilds it from portable plaintext rather than copying ciphertext.
    let portable = |v: Vec<(String, Vec<u8>)>| -> Vec<(String, Vec<u8>)> {
        v.into_iter()
            .filter(|(k, _)| {
                k != "Local State" && k != "Default/Cookies" && k != "Default/Network/Cookies"
            })
            .collect()
    };
    let (pb, pa) = (portable(before), portable(after));
    let names = |v: &[(String, Vec<u8>)]| v.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    assert_eq!(names(&pb), names(&pa), "restored file set differs");
    for ((kb, vb), (_, va)) in pb.iter().zip(pa.iter()) {
        assert_eq!(vb, va, "content differs for {kb}");
    }
    // `Local State` at the destination must be the destination's own, never a
    // copy of the source's: unpack carries over the live file, and on Windows
    // mints one so cookies can be re-sealed with this machine's key.
    if dst.join("Local State").exists() {
        assert_ne!(
            fs::read(dst.join("Local State")).unwrap(),
            fs::read(src.join("Local State")).unwrap(),
            "the source machine key must not travel in a backup"
        );
    }
    assert!(
        dst.join("Default/Cookies").exists(),
        "unpack must rebuild the Cookies DB with the destination key"
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn the_ciphertext_does_not_contain_the_plaintext() {
    // Guards against the container accidentally storing profile bytes in the
    // clear — the single most damaging way this could silently be wrong.
    let base = scratch("ciphertext");
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();
    let marker = b"SENTINEL-SECRET-COOKIE-VALUE-9d3f";
    seed_profile(&src);
    // Placed in a file the packer carries, so the assertion is about the
    // ciphertext and not about an exclusion rule.
    fs::write(src.join("Default/Local Storage/secret.bin"), marker).unwrap();

    let file = base.join("backup.shxbak");
    backup_file::create("profile-1", &src, &file, PASSPHRASE).expect("create backup");
    let bytes = fs::read(&file).unwrap();

    assert!(
        !bytes.windows(marker.len()).any(|w| w == marker),
        "plaintext marker found in the backup file"
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn a_wrong_passphrase_is_refused_and_leaves_the_destination_untouched() {
    let base = scratch("wrongpass");
    let src = base.join("src");
    let dst = base.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    seed_profile(&src);
    // Pre-existing content in the destination must survive a failed restore.
    fs::write(dst.join("do-not-delete.txt"), b"keep me").unwrap();

    let file = base.join("backup.shxbak");
    backup_file::create("profile-1", &src, &file, PASSPHRASE).expect("create backup");

    let err = backup_file::restore(&file, &dst, "a-different-passphrase")
        .expect_err("a wrong passphrase must fail");
    assert!(
        err.to_string().contains("wrong passphrase"),
        "error should name the likely cause, got: {err}"
    );
    assert_eq!(
        fs::read(dst.join("do-not-delete.txt")).unwrap(),
        b"keep me",
        "a failed restore must not disturb the destination"
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn a_truncated_backup_is_refused_and_leaves_the_destination_untouched() {
    // This is the case the P-OP drill measured: `backup::open` streams
    // authenticated plaintext before it reaches the signed head that reveals
    // truncation. `restore` must recover in memory first, so the profile is
    // never partially written.
    let base = scratch("truncated");
    let src = base.join("src");
    let dst = base.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    seed_profile(&src);
    fs::write(dst.join("do-not-delete.txt"), b"keep me").unwrap();

    let file = base.join("backup.shxbak");
    backup_file::create("profile-1", &src, &file, PASSPHRASE).expect("create backup");

    let mut bytes = fs::read(&file).unwrap();
    bytes.truncate(bytes.len() - 64);
    fs::write(&file, &bytes).unwrap();

    backup_file::restore(&file, &dst, PASSPHRASE).expect_err("a truncated backup must fail");
    assert_eq!(
        fs::read(dst.join("do-not-delete.txt")).unwrap(),
        b"keep me",
        "a truncated restore must not disturb the destination"
    );
    assert!(
        !dst.join("Default").exists(),
        "no profile content may be written from a backup that failed to verify"
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn a_tampered_backup_body_is_refused() {
    let base = scratch("tampered");
    let src = base.join("src");
    let dst = base.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    seed_profile(&src);

    let file = base.join("backup.shxbak");
    backup_file::create("profile-1", &src, &file, PASSPHRASE).expect("create backup");

    let mut bytes = fs::read(&file).unwrap();
    // Flip a bit well inside the sealed body, past the header.
    let idx = bytes.len() / 2;
    bytes[idx] ^= 0x01;
    fs::write(&file, &bytes).unwrap();

    backup_file::restore(&file, &dst, PASSPHRASE).expect_err("a tampered backup must fail");
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn a_non_backup_file_is_rejected_before_the_passphrase_is_used() {
    let base = scratch("notbackup");
    let file = base.join("random.bin");
    fs::write(&file, vec![0u8; 4096]).unwrap();

    let err = backup_file::inspect(&file).expect_err("inspect must reject a non-backup");
    assert!(err.to_string().contains("not a ShardX backup"));

    let dst = base.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let err = backup_file::restore(&file, &dst, PASSPHRASE).expect_err("restore must reject");
    assert!(err.to_string().contains("not a ShardX backup"));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn a_short_passphrase_is_refused_at_backup_time() {
    let base = scratch("shortpass");
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();
    seed_profile(&src);

    let err = backup_file::create("profile-1", &src, &base.join("b.shxbak"), "short")
        .expect_err("a short passphrase must be refused");
    assert!(err.to_string().contains("at least"));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn two_backups_of_the_same_profile_differ() {
    // Same passphrase and same input must still produce different files: the
    // salt and per-backup secrets are random, so identical output would mean a
    // reused keystream.
    let base = scratch("distinct");
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();
    seed_profile(&src);

    let a = base.join("a.shxbak");
    let b = base.join("b.shxbak");
    let ia = backup_file::create("profile-1", &src, &a, PASSPHRASE).expect("a");
    let ib = backup_file::create("profile-1", &src, &b, PASSPHRASE).expect("b");

    assert_ne!(ia.sha256, ib.sha256, "two backups must not be identical");
    assert!(
        backup_file::restore(&a, &base.join("da"), PASSPHRASE).is_ok(),
        "first backup must still open"
    );
    assert!(
        backup_file::restore(&b, &base.join("db"), PASSPHRASE).is_ok(),
        "second backup must still open"
    );
    let _ = fs::remove_dir_all(&base);
}
