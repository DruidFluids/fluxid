//! Access to the embedded Flux widget payload.
//!
//! `build.rs` produces `OUT_DIR/payload.bin` — either the real `flux.exe`
//! **gzip-compressed** (packaged build) or an empty placeholder (plain
//! `cargo build`). Storing it compressed keeps a raw embedded PE — the classic
//! "dropper" shape AV heuristics flag — out of the unsigned installer; we
//! decompress to the original bytes only when writing it to disk at install time.

/// The embedded, gzip-compressed `flux.exe`, or empty in a dev build.
const FLUX_EXE_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));

/// Uncompressed size of the payload, baked in by `build.rs` (0 in a dev build).
fn raw_len() -> usize {
    env!("FLUX_RAW_LEN").parse().unwrap_or(0)
}

/// Whether a real payload was embedded. `false` in a dev build, in which case
/// the installer refuses to run the file-copy step.
pub fn is_bundled() -> bool {
    !FLUX_EXE_GZ.is_empty()
}

/// Human-readable size of the installed `flux.exe` (the uncompressed payload).
pub fn size_mb() -> f32 {
    raw_len() as f32 / (1024.0 * 1024.0)
}

/// Decompress the embedded payload back to the original `flux.exe` bytes,
/// byte-for-byte identical to what was packaged (verified at build time).
pub fn flux_exe() -> Vec<u8> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut out = Vec::with_capacity(raw_len());
    GzDecoder::new(FLUX_EXE_GZ)
        .read_to_end(&mut out)
        .expect("failed to decompress embedded Flux payload");
    out
}
