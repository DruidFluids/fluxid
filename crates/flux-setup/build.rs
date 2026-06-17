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
use std::path::{Path, PathBuf};

fn main() {
    // Embed the Windows resources: icon, version-info, and an `asInvoker` manifest.
    //
    // The manifest is load-bearing: without it Windows' installer-detection
    // heuristic auto-flags any exe named "...setup..." as requiring elevation, so a
    // non-elevated widget can't launch the installer for an in-app self-update —
    // CreateProcess fails with ERROR_ELEVATION_REQUIRED (os error 740). The install
    // is per-user (%LOCALAPPDATA% + HKCU), so no admin is needed; `asInvoker` opts
    // out of the heuristic and runs with no UAC prompt.
    //
    // Proper version-info + an icon also matter: a metadata-less, unsigned, single-
    // file installer is exactly the shape antivirus ML heuristics over-flag, so
    // giving it real file metadata makes it look like the legitimate app it is.
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_windows_resources();
    }

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

/// Embed the icon, version-info, and `asInvoker` manifest into flux-setup.exe.
/// The manifest is mandatory (a missing one re-introduces the elevation bug), so
/// a resource-compile failure is fatal rather than best-effort.
fn embed_windows_resources() {
    println!("cargo:rerun-if-changed=assets/icon.png");
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let ico = out_dir.join("Flux.ico");
    let have_icon = match generate_ico("assets/icon.png", &ico) {
        Ok(()) => true,
        Err(e) => {
            println!("cargo:warning=could not build Flux.ico ({e}); installer will have no icon");
            false
        }
    };

    let ver = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut parts = ver.split('.').map(|s| s.parse::<u64>().unwrap_or(0));
    let (maj, min, pat) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    let packed = (maj << 48) | (min << 32) | (pat << 16);

    const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#;

    let mut res = winresource::WindowsResource::new();
    if have_icon {
        res.set_icon(&ico.to_string_lossy());
    }
    res.set("ProductName", "Flux")
        .set("FileDescription", "Flux Installer")
        .set("CompanyName", "DruidFluids")
        .set("LegalCopyright", "Copyright (c) 2026 Matt Hakes")
        .set("OriginalFilename", "flux-setup.exe")
        .set("InternalName", "flux-setup")
        .set("FileVersion", ver.as_str())
        .set("ProductVersion", ver.as_str());
    res.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
    res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
    res.set_manifest(MANIFEST);
    res.compile().expect("failed to embed flux-setup resources (icon/version/manifest)");
}

/// Build a multi-size `.ico` from the source PNG (mirrors flux-widget's build.rs).
fn generate_ico(png: &str, ico_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let src = image::open(png)?;
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 32, 48, 64, 128, 256] {
        let rgba = src
            .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let img = ico::IconImage::from_rgba_data(size, size, rgba.into_raw());
        dir.add_entry(ico::IconDirEntry::encode(&img)?);
    }
    dir.write(fs::File::create(ico_path)?)?;
    Ok(())
}
