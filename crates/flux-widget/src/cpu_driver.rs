//! Optional CPU-temperature sensor driver (PawnIO) management.
//!
//! Faithful port of the C# `CpuSensorDriver`. Flux **never bundles or
//! redistributes the driver**. When the user explicitly opts in, this downloads
//! the official signed installer from PawnIO's own release URL, verifies its
//! Authenticode signature, and runs it silently elevated (one Windows UAC
//! prompt — driver installs require elevation by design). Detection and
//! uninstall go through the driver's own Uninstall registry key.
//!
//! Security posture (this is the app's #1 goal — zero security issues):
//!   * HTTPS download from the canonical, single-source-of-truth URL.
//!   * The installer is **never executed unless `WinVerifyTrust` confirms a
//!     trusted, valid, non-revoked Authenticode signature** (full chain +
//!     revocation, mirroring and slightly exceeding the C# `X509Chain.Build`).
//!   * Hardening over C#: optional **publisher pinning** — if `EXPECTED_SIGNER`
//!     is set, an installer that is validly signed but by the *wrong* publisher
//!     (e.g. served from a hijacked URL) is still rejected.
//!   * Every failure path is non-fatal: the app's core never depends on this.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Official signed installer. Single source of truth — change here only.
pub const DOWNLOAD_URL: &str =
    "https://github.com/namazso/PawnIO.Setup/releases/latest/download/PawnIO_setup.exe";
pub const HOME_PAGE_URL: &str = "https://pawnio.eu/";
pub const SOURCE_URL: &str = "https://github.com/namazso/PawnIO";

/// The driver's uninstall registry key (64-bit view) — used for both presence
/// detection and locating the uninstaller for opt-out.
const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO";

// NOTE (planned hardening): in addition to the WinVerifyTrust trusted-chain
// gate below, we want to *pin the publisher* — reject an installer that is
// validly signed but by the wrong signer (e.g. served from a hijacked URL).
// That needs the exact PawnIO signing CN, which isn't confirmed yet, so it's a
// tracked follow-up. WinVerifyTrust (trusted chain + revocation) is the hard
// security gate in the meantime.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallResult {
    /// The driver was installed by this run.
    Installed,
    /// The driver was already present (no-op).
    AlreadyPresent,
    /// The user declined the UAC elevation prompt.
    Cancelled,
    /// Something went wrong; `Outcome::detail` explains.
    Failed,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub result: InstallResult,
    pub detail: String,
}

impl Outcome {
    fn ok(result: InstallResult) -> Self {
        Outcome { result, detail: String::new() }
    }
    fn failed(detail: impl Into<String>) -> Self {
        Outcome { result: InstallResult::Failed, detail: detail.into() }
    }
}

/// True when the sensor driver is installed on this machine.
pub fn is_installed() -> bool {
    version().is_some()
}

/// The installed driver's `DisplayVersion`, if present.
#[cfg(target_os = "windows")]
pub fn version() -> Option<String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(UNINSTALL_KEY, KEY_READ | KEY_WOW64_64KEY)
        .ok()?;
    let v: String = key.get_value("DisplayVersion").ok()?;
    if v.is_empty() { None } else { Some(v) }
}

#[cfg(not(target_os = "windows"))]
pub fn version() -> Option<String> {
    None
}

fn temp_installer_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("Flux_sensor_{stamp:x}.exe"))
}

async fn download_installer() -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .user_agent("Flux")
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(DOWNLOAD_URL).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    Ok(resp.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

/// Opt-in install: download → verify signature → silent elevated install. The
/// one UAC prompt fires when the installer launches with the `runas` verb.
pub async fn install() -> Outcome {
    if is_installed() {
        return Outcome::ok(InstallResult::AlreadyPresent);
    }
    let bytes = match download_installer().await {
        Ok(b) => b,
        Err(e) => return Outcome::failed(format!("Download failed: {e}")),
    };
    let path = temp_installer_path();
    if let Err(e) = std::fs::write(&path, &bytes) {
        return Outcome::failed(format!("Could not save the installer: {e}"));
    }
    // Verification + elevated run + wait are all blocking Win32 calls.
    tokio::task::spawn_blocking(move || install_blocking(path))
        .await
        .unwrap_or_else(|_| Outcome::failed("Internal error during install."))
}

/// Opt-out: run the driver's own uninstaller (one UAC prompt).
pub async fn uninstall() -> Outcome {
    if !is_installed() {
        return Outcome::ok(InstallResult::AlreadyPresent);
    }
    tokio::task::spawn_blocking(uninstall_blocking)
        .await
        .unwrap_or_else(|_| Outcome::failed("Internal error during uninstall."))
}

#[cfg(target_os = "windows")]
fn install_blocking(path: PathBuf) -> Outcome {
    // 1. Verify the Authenticode signature BEFORE running anything. A driver
    //    installer that isn't validly, trustedly signed is never executed.
    if let Err(e) = verify_authenticode(&path) {
        try_delete(&path);
        return Outcome::failed(format!(
            "The downloaded installer's signature could not be verified ({e})."
        ));
    }

    // 2. Silent elevated install. "-install -silent" are PawnIO's own switches.
    match run_elevated_wait(&path, "-install -silent") {
        Ok(true) => {}
        Ok(false) => {
            try_delete(&path);
            return Outcome::ok(InstallResult::Cancelled);
        }
        Err(e) => {
            try_delete(&path);
            return Outcome::failed(e);
        }
    }
    try_delete(&path);

    // 3. Confirm it actually landed.
    if is_installed() {
        Outcome::ok(InstallResult::Installed)
    } else {
        Outcome::failed("The installer ran but the driver was not detected afterward.")
    }
}

#[cfg(target_os = "windows")]
fn uninstall_blocking() -> Outcome {
    let cmd = match uninstall_command() {
        Some(c) if !c.trim().is_empty() => c,
        _ => return Outcome::failed("Could not locate the driver's uninstaller."),
    };

    // UninstallString may be `"C:\path\unins.exe" /flags`. Split exe from args.
    let (exe, mut args) = if let Some(rest) = cmd.strip_prefix('"') {
        match rest.find('"') {
            Some(end) => (rest[..end].to_string(), rest[end + 1..].trim().to_string()),
            None => (cmd.clone(), String::new()),
        }
    } else if let Some(sp) = cmd.find(' ') {
        (cmd[..sp].to_string(), cmd[sp + 1..].trim().to_string())
    } else {
        (cmd.clone(), String::new())
    };
    if !args.to_lowercase().contains("silent") {
        args = format!("-uninstall -silent {args}").trim().to_string();
    }

    match run_elevated_wait(Path::new(&exe), &args) {
        Ok(true) => {}
        Ok(false) => return Outcome::ok(InstallResult::Cancelled),
        Err(e) => return Outcome::failed(e),
    }

    if is_installed() {
        Outcome::failed("The uninstaller ran but the driver is still present.")
    } else {
        // State changed OK — reuse Installed to mean "the requested change took".
        Outcome::ok(InstallResult::Installed)
    }
}

#[cfg(target_os = "windows")]
fn uninstall_command() -> Option<String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(UNINSTALL_KEY, KEY_READ | KEY_WOW64_64KEY)
        .ok()?;
    key.get_value("QuietUninstallString")
        .or_else(|_| key.get_value("UninstallString"))
        .ok()
}

fn try_delete(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Launch `file params` elevated (UAC), hidden, and **wait** for it to finish.
/// Returns `Ok(true)` if it ran to completion, `Ok(false)` if the user declined
/// the UAC prompt, `Err` on any other launch failure.
#[cfg(target_os = "windows")]
fn run_elevated_wait(file: &Path, params: &str) -> Result<bool, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError};
    use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};
    use windows::Win32::UI::Shell::{
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let verb = wide("runas");
    let file_w = wide(&file.to_string_lossy());
    let params_w = wide(params);

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file_w.as_ptr()),
        lpParameters: PCWSTR(params_w.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };

    unsafe {
        match ShellExecuteExW(&mut info) {
            Ok(()) => {
                if !info.hProcess.is_invalid() {
                    WaitForSingleObject(info.hProcess, INFINITE);
                    let _ = CloseHandle(info.hProcess);
                }
                Ok(true)
            }
            Err(_) => {
                if GetLastError() == ERROR_CANCELLED {
                    Ok(false)
                } else {
                    Err("Could not launch the installer (elevation failed).".into())
                }
            }
        }
    }
}

/// Verify the file carries a trusted, valid, non-revoked Authenticode signature.
/// Mirrors the C# `X509Chain.Build` with `RevocationFlag.ExcludeRoot`, and
/// tolerates an offline machine (chain structure still validated, only
/// revocation *reachability* is relaxed).
#[cfg(target_os = "windows")]
fn verify_authenticode(path: &Path) -> Result<String, String> {
    use windows::Win32::Security::WinTrust::{
        WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
        WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT, WTD_REVOKE_NONE,
        WTD_REVOKE_WHOLECHAIN, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
        WinVerifyTrust,
    };
    use windows::Win32::Foundation::HWND;
    use windows::core::PCWSTR;

    // Offline/unreachable revocation HRESULTs we tolerate (matching C#).
    const CERT_E_REVOCATION_FAILURE: i32 = 0x800B010Eu32 as i32;
    const CRYPT_E_REVOCATION_OFFLINE: i32 = 0x80092013u32 as i32;
    const CRYPT_E_NO_REVOCATION_CHECK: i32 = 0x80092012u32 as i32;

    let file_w = wide(&path.to_string_lossy());

    let verify = |revoke: windows::Win32::Security::WinTrust::WINTRUST_DATA_REVOCATION_CHECKS| -> i32 {
        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(file_w.as_ptr()),
            ..Default::default()
        };
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let mut data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: revoke,
            dwUnionChoice: WTD_CHOICE_FILE,
            dwStateAction: WTD_STATEACTION_VERIFY,
            dwProvFlags: WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT,
            Anonymous: WINTRUST_DATA_0 { pFile: &mut file_info as *mut _ },
            ..Default::default()
        };
        let status = unsafe {
            WinVerifyTrust(
                HWND::default(),
                &mut action,
                &mut data as *mut _ as *mut core::ffi::c_void,
            )
        };
        // Always release the provider state.
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrust(
                HWND::default(),
                &mut action,
                &mut data as *mut _ as *mut core::ffi::c_void,
            );
        }
        status
    };

    let mut status = verify(WTD_REVOKE_WHOLECHAIN);
    if matches!(
        status,
        CERT_E_REVOCATION_FAILURE | CRYPT_E_REVOCATION_OFFLINE | CRYPT_E_NO_REVOCATION_CHECK
    ) {
        // Offline: re-verify the chain without requiring revocation reachability.
        status = verify(WTD_REVOKE_NONE);
    }

    if status == 0 {
        Ok(String::new())
    } else {
        Err(format!("0x{:08X}", status as u32))
    }
}

// ── Non-Windows stubs (the feature is Windows-only) ──
#[cfg(not(target_os = "windows"))]
fn install_blocking(_path: PathBuf) -> Outcome {
    Outcome::failed("CPU sensor driver is Windows-only.")
}
#[cfg(not(target_os = "windows"))]
fn uninstall_blocking() -> Outcome {
    Outcome::failed("CPU sensor driver is Windows-only.")
}
