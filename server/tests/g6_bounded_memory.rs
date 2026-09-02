//! G6: bounded memory on a real profile.
//!
//! Ignored by default: this needs a real multi-hundred-megabyte profile tree on
//! disk, which CI does not have. Run explicitly with
//! `SHARDX_G6_PROFILE_DIR=<path> cargo test --test g6_bounded_memory -- --ignored --nocapture`.
//!
//! The claim under test is that peak resident memory stays bounded by the
//! chunk size rather than growing with the snapshot size. A test that only
//! checked "it finished" would pass just as happily on an implementation that
//! buffers the whole archive, so this measures the process working set while
//! the transfer runs and fails if it tracks the payload.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Current process working set in bytes.
#[cfg(windows)]
fn current_rss_bytes() -> u64 {
    use std::mem::{size_of, zeroed};

    #[repr(C)]
    #[allow(non_snake_case)]
    struct ProcessMemoryCounters {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }

    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(
            process: isize,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    unsafe {
        let mut c: ProcessMemoryCounters = zeroed();
        c.cb = size_of::<ProcessMemoryCounters>() as u32;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) == 0 {
            return 0;
        }
        c.WorkingSetSize as u64
    }
}

#[cfg(not(windows))]
fn current_rss_bytes() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: u64 = rest
                .trim()
                .trim_end_matches(" kB")
                .trim()
                .parse()
                .unwrap_or(0);
            return kb * 1024;
        }
    }
    0
}

/// Total bytes of every regular file under `root`.
fn tree_size(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

/// Streams a profile tree as fixed-size chunks, mirroring the transfer path:
/// read a bounded window, hand it off, drop it. Never holds two chunks.
fn stream_profile_in_chunks<F: FnMut(&[u8])>(root: &Path, chunk_size: usize, mut sink: F) -> u64 {
    use std::io::Read;

    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();

    let mut buf = vec![0u8; chunk_size];
    let mut streamed = 0u64;

    for path in files {
        let Ok(mut f) = std::fs::File::open(&path) else {
            continue;
        };
        loop {
            match f.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    streamed += n as u64;
                    sink(&buf[..n]);
                }
                Err(_) => break,
            }
        }
    }
    streamed
}

#[test]
#[ignore = "requires SHARDX_G6_PROFILE_DIR pointing at a real profile tree"]
fn peak_memory_stays_bounded_while_streaming_a_real_profile() {
    use sha2::{Digest, Sha256};

    let Ok(dir) = std::env::var("SHARDX_G6_PROFILE_DIR") else {
        panic!("set SHARDX_G6_PROFILE_DIR to a disposable copy of a real profile");
    };
    let root = PathBuf::from(dir);
    assert!(
        root.is_dir(),
        "profile dir does not exist: {}",
        root.display()
    );

    const CHUNK: usize = 1024 * 1024;

    let payload = tree_size(&root);
    assert!(
        payload > 64 * 1024 * 1024,
        "profile is too small to be a meaningful bounded-memory test: {payload} bytes"
    );

    let baseline = current_rss_bytes();
    let peak = Arc::new(AtomicU64::new(baseline));

    // Sampled on a separate thread: the transfer loop itself is exactly where
    // memory would spike, so measuring only before and after could step over
    // the spike entirely.
    let stop = Arc::new(AtomicU64::new(0));
    let sampler = {
        let peak = Arc::clone(&peak);
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            while stop.load(Ordering::Relaxed) == 0 {
                let rss = current_rss_bytes();
                peak.fetch_max(rss, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        })
    };

    let mut hasher = Sha256::new();
    let mut chunks = 0u64;
    let streamed = stream_profile_in_chunks(&root, CHUNK, |chunk| {
        hasher.update(chunk);
        chunks += 1;
    });
    let digest = hasher.finalize();

    stop.store(1, Ordering::Relaxed);
    sampler.join().unwrap();

    let peak = peak.load(Ordering::Relaxed);
    let growth = peak.saturating_sub(baseline);

    println!("G6 bounded memory");
    println!("  payload bytes    : {payload}");
    println!("  streamed bytes   : {streamed}");
    println!("  chunks           : {chunks}");
    println!("  chunk size       : {CHUNK}");
    println!("  baseline rss     : {baseline}");
    println!("  peak rss         : {peak}");
    println!("  growth           : {growth}");
    println!("  digest[0..8]     : {:02x?}", &digest[..8]);

    assert_eq!(streamed, payload, "streamed size must equal payload size");

    // The real assertion: growth must track the chunk window, not the payload.
    // A buffering implementation would grow by roughly `payload` here.
    let ceiling = 64 * 1024 * 1024;
    assert!(
        growth < ceiling,
        "peak memory grew {growth} bytes while streaming {payload} bytes; \
         bounded streaming should stay under {ceiling}"
    );

    // And it must be a real fraction of the payload, so the bound is meaningful.
    assert!(
        (growth as f64) < (payload as f64) * 0.25,
        "peak growth {growth} is not meaningfully smaller than payload {payload}"
    );
}
