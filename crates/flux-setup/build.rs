//! Embeds the Flux payload (the widget exe) into the installer at build time.
//!
//! The packaging script (`scripts/Build-Setup.ps1`) release-builds `flux.exe`
//! and points `FLUX_PAYLOAD` at it before building this crate. We copy that
//! file to `OUT_DIR/payload.bin`, which `payload.rs` pulls in with
//! `include_bytes!`. When the variable is unset (a plain `cargo build` during
//! development) we write a zero-byte placeholder so the workspace still compiles
//! — the installer then detects the empty payload and refuses to install,
//! rather than the build failing outright.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let dest = out_dir.join("payload.bin");

    println!("cargo:rerun-if-env-changed=FLUX_PAYLOAD");

    match env::var_os("FLUX_PAYLOAD") {
        Some(src) if !src.is_empty() => {
            let src = PathBuf::from(src);
            println!("cargo:rerun-if-changed={}", src.display());
            match fs::copy(&src, &dest) {
                Ok(n) => {
                    println!(
                        "cargo:warning=Embedded Flux payload ({n} bytes) from {}",
                        src.display()
                    );
                }
                Err(e) => panic!(
                    "FLUX_PAYLOAD is set to {} but it could not be read: {e}",
                    src.display()
                ),
            }
        }
        _ => {
            // No payload supplied — write an empty placeholder so the build still
            // succeeds. The installer treats an empty payload as "dev build".
            fs::write(&dest, []).expect("failed to write placeholder payload.bin");
            println!(
                "cargo:warning=FLUX_PAYLOAD not set — building installer with an EMPTY payload (dev build, cannot install)."
            );
        }
    }
}
