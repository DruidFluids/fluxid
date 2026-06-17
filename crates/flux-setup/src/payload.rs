//! Access to the embedded Flux widget payload.
//!
//! `build.rs` produces `OUT_DIR/payload.bin` — either the real `flux.exe`
//! (packaged build) or an empty placeholder (plain `cargo build`).

/// The embedded `flux.exe`, or empty in a dev build (see `build.rs`).
pub const FLUX_EXE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));

/// Whether a real payload was embedded. `false` in a dev build, in which case
/// the installer refuses to run the file-copy step.
pub fn is_bundled() -> bool {
    !FLUX_EXE.is_empty()
}

/// Human-readable size of the embedded payload (for the UI).
pub fn size_mb() -> f32 {
    FLUX_EXE.len() as f32 / (1024.0 * 1024.0)
}
