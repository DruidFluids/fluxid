//! The actual install / uninstall work — pure logic, no UI.
//!
//! Runs headless. The GUI wizard calls [`install`] in-process for a per-user
//! install; for an all-users install it relaunches the installer elevated with
//! `--apply` so this code runs inside the elevated process. The uninstaller
//! (`--uninstall`) is a copy of this same exe placed in the install dir and
//! registered as the Add/Remove-Programs uninstall command.

use std::fmt;
use std::path::{Path, PathBuf};

/// Display name everywhere the user sees it, and the registry value name.
pub const APP_NAME: &str = "Flux";
/// Filename of the installed widget.
pub const EXE_NAME: &str = "flux.exe";
/// Filename the installer copies itself to so it can act as the uninstaller.
pub const UNINSTALL_EXE: &str = "uninstall.exe";
pub const PUBLISHER: &str = "Matt Hakes";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// HKCU value the widget itself also writes — keep in sync with flux-widget.
const RUN_VALUE: &str = "Flux";
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const UNINSTALL_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Flux";

/// Where Flux is installed — and therefore which registry hive / shell
/// folders are used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `%LOCALAPPDATA%\Flux`, HKCU, no elevation required.
    PerUser,
    /// `%ProgramFiles%\Flux`, HKLM, requires elevation.
    AllUsers,
}

impl Scope {
    pub fn as_flag(self) -> &'static str {
        match self {
            Scope::PerUser => "per-user",
            Scope::AllUsers => "all-users",
        }
    }
    pub fn parse(s: &str) -> Option<Scope> {
        match s {
            "per-user" => Some(Scope::PerUser),
            "all-users" => Some(Scope::AllUsers),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InstallOptions {
    pub scope: Scope,
    pub desktop_shortcut: bool,
    pub run_at_startup: bool,
    pub launch_after: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct UninstallOptions {
    pub scope: Scope,
    /// Also delete `%APPDATA%\Flux` (user settings, themes, skins).
    pub remove_settings: bool,
}

/// A step-by-step log the GUI can show; also handy for headless logging.
#[derive(Debug, Default)]
pub struct Report {
    pub steps: Vec<String>,
}
impl Report {
    fn step(&mut self, msg: impl Into<String>) {
        self.steps.push(msg.into());
    }
}

#[derive(Debug)]
pub struct EngineError(pub String);
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EngineError {}

type Result<T> = std::result::Result<T, EngineError>;

fn err(msg: impl Into<String>) -> EngineError {
    EngineError(msg.into())
}

// ───────────────────────────── Windows impl ─────────────────────────────

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    pub fn is_elevated() -> bool {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            )
            .is_ok();
            let _ = CloseHandle(token);
            ok && elevation.TokenIsElevated != 0
        }
    }

    /// Relaunch this exe elevated with the given args, hidden, and wait. Returns
    /// `Ok(Some(code))` with the child's exit code, `Ok(None)` if the user
    /// declined the UAC prompt, `Err` on any other launch failure.
    pub fn relaunch_elevated_wait(args: &[String]) -> Result<Option<i32>> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError};
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, WaitForSingleObject, INFINITE,
        };
        use windows::Win32::UI::Shell::{
            ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        };
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let exe = std::env::current_exe().map_err(|e| err(format!("current_exe: {e}")))?;
        let params = args.join(" ");

        let verb = wide("runas");
        let file_w = wide(&exe.to_string_lossy());
        let params_w = wide(&params);

        unsafe {
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS,
                lpVerb: PCWSTR(verb.as_ptr()),
                lpFile: PCWSTR(file_w.as_ptr()),
                lpParameters: PCWSTR(params_w.as_ptr()),
                nShow: SW_HIDE.0,
                ..Default::default()
            };
            match ShellExecuteExW(&mut info) {
                Ok(()) => {
                    if info.hProcess.is_invalid() {
                        return Ok(Some(0));
                    }
                    WaitForSingleObject(info.hProcess, INFINITE);
                    let mut code = 0u32;
                    let _ = GetExitCodeProcess(info.hProcess, &mut code);
                    let _ = CloseHandle(info.hProcess);
                    Ok(Some(code as i32))
                }
                Err(_) => {
                    if GetLastError() == ERROR_CANCELLED {
                        Ok(None)
                    } else {
                        Err(err("Could not start the elevated installer."))
                    }
                }
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn known_folder(rfid: *const windows::core::GUID) -> Result<PathBuf> {
        use windows::Win32::System::Com::CoTaskMemFree;
        use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};

        unsafe {
            let pwstr = SHGetKnownFolderPath(rfid, KF_FLAG_DEFAULT, None)
                .map_err(|e| err(format!("SHGetKnownFolderPath: {e}")))?;
            let s = pwstr
                .to_string()
                .map_err(|e| err(format!("known folder path decode: {e}")))?;
            CoTaskMemFree(Some(pwstr.0 as *const core::ffi::c_void));
            Ok(PathBuf::from(s))
        }
    }

    pub fn install_dir(scope: Scope) -> Result<PathBuf> {
        use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, FOLDERID_ProgramFiles};
        let base = match scope {
            Scope::PerUser => known_folder(&FOLDERID_LocalAppData)?,
            Scope::AllUsers => known_folder(&FOLDERID_ProgramFiles)?,
        };
        Ok(base.join(APP_NAME))
    }

    fn start_menu_dir(scope: Scope) -> Result<PathBuf> {
        use windows::Win32::UI::Shell::{FOLDERID_CommonPrograms, FOLDERID_Programs};
        match scope {
            Scope::PerUser => known_folder(&FOLDERID_Programs),
            Scope::AllUsers => known_folder(&FOLDERID_CommonPrograms),
        }
    }

    fn desktop_dir(scope: Scope) -> Result<PathBuf> {
        use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_PublicDesktop};
        match scope {
            Scope::PerUser => known_folder(&FOLDERID_Desktop),
            Scope::AllUsers => known_folder(&FOLDERID_PublicDesktop),
        }
    }

    /// `%APPDATA%\Flux` — the widget's per-user settings/themes/skins dir.
    fn settings_dir() -> Result<PathBuf> {
        use windows::Win32::UI::Shell::FOLDERID_RoamingAppData;
        Ok(known_folder(&FOLDERID_RoamingAppData)?.join(APP_NAME))
    }

    fn create_shortcut(
        lnk_path: &std::path::Path,
        target: &std::path::Path,
        working_dir: &std::path::Path,
        description: &str,
    ) -> Result<()> {
        use windows::core::{Interface, HSTRING};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile,
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

        unsafe {
            // COM must be live on this thread for CoCreateInstance. If a
            // different mode is already initialised (RPC_E_CHANGED_MODE) COM is
            // still usable — we just don't own it, so we only uninit when we
            // were the ones that initialised it.
            let owns_com = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();

            let build = || -> Result<()> {
                let link: IShellLinkW =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| err(format!("create ShellLink: {e}")))?;
                link.SetPath(&HSTRING::from(target.as_os_str()))
                    .map_err(|e| err(format!("SetPath: {e}")))?;
                link.SetWorkingDirectory(&HSTRING::from(working_dir.as_os_str()))
                    .map_err(|e| err(format!("SetWorkingDirectory: {e}")))?;
                link.SetDescription(&HSTRING::from(description))
                    .map_err(|e| err(format!("SetDescription: {e}")))?;
                // Icon comes from the exe's own embedded icon (index 0).
                link.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)
                    .map_err(|e| err(format!("SetIconLocation: {e}")))?;
                let persist: IPersistFile = link
                    .cast()
                    .map_err(|e| err(format!("cast IPersistFile: {e}")))?;
                persist
                    .Save(&HSTRING::from(lnk_path.as_os_str()), true)
                    .map_err(|e| err(format!("save shortcut: {e}")))?;
                Ok(())
            };

            let result = build();
            if owns_com {
                CoUninitialize();
            }
            result
        }
    }

    // ── Registry (Add/Remove Programs + startup) ──

    fn root_key(scope: Scope) -> winreg::RegKey {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;
        match scope {
            Scope::PerUser => RegKey::predef(HKEY_CURRENT_USER),
            Scope::AllUsers => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }

    fn write_arp_entry(scope: Scope, dir: &std::path::Path, size_kb: u32) -> Result<()> {
        let exe = dir.join(EXE_NAME);
        let uninstaller = dir.join(UNINSTALL_EXE);
        let uninstall_cmd =
            format!("\"{}\" --uninstall --scope {}", uninstaller.display(), scope.as_flag());
        let quiet_cmd = format!("{uninstall_cmd} --silent");

        let (key, _) = root_key(scope)
            .create_subkey(UNINSTALL_SUBKEY)
            .map_err(|e| err(format!("create uninstall key: {e}")))?;

        let set = |name: &str, val: &str| -> Result<()> {
            key.set_value(name, &val.to_string())
                .map_err(|e| err(format!("set {name}: {e}")))
        };
        set("DisplayName", APP_NAME)?;
        set("DisplayVersion", VERSION)?;
        set("Publisher", PUBLISHER)?;
        set("DisplayIcon", &exe.to_string_lossy())?;
        set("InstallLocation", &dir.to_string_lossy())?;
        set("UninstallString", &uninstall_cmd)?;
        set("QuietUninstallString", &quiet_cmd)?;
        set("URLInfoAbout", "https://github.com/DruidFluids/Flux")?;
        key.set_value("EstimatedSize", &size_kb)
            .map_err(|e| err(format!("set EstimatedSize: {e}")))?;
        key.set_value("NoModify", &1u32).ok();
        key.set_value("NoRepair", &1u32).ok();
        Ok(())
    }

    fn delete_arp_entry(scope: Scope) {
        let _ = root_key(scope).delete_subkey_all(UNINSTALL_SUBKEY);
    }

    fn set_run_at_startup(exe: &std::path::Path, on: bool) -> Result<()> {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey(RUN_SUBKEY)
            .map_err(|e| err(format!("open Run key: {e}")))?;
        // Always clear pre-rename / stale Run entries so the fluxid→Flux upgrade
        // doesn't leave a duplicate autostart pointing at the old exe.
        let _ = key.delete_value("fluxid");
        let _ = key.delete_value("Fluxid");
        let _ = key.delete_value("fluidMonitor");
        if on {
            key.set_value(RUN_VALUE, &exe.to_string_lossy().to_string())
                .map_err(|e| err(format!("set Run value: {e}")))?;
        } else {
            let _ = key.delete_value(RUN_VALUE);
        }
        Ok(())
    }

    fn kill_running_widget() {
        // Force-kill before touching files (mirrors the C# uninstaller): a
        // graceful close re-saves settings and would also lock the exe. Kill the
        // old "fluxid.exe" too so a live fluxid→Flux upgrade replaces it cleanly.
        for name in [EXE_NAME, "fluxid.exe"] {
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", name])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }

    /// True when the optional CPU-sensor service is currently running. When it
    /// is, it holds `flux.exe` open (it runs `flux.exe --sensor-service`), which
    /// blocks an in-place self-update — the caller must stop it first (needs
    /// elevation), which is why `run_apply_cli` relaunches elevated in that case.
    pub fn sensor_service_running() -> bool {
        Command::new("sc")
            .args(["query", "FluxSensorService"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("RUNNING"))
            .unwrap_or(false)
    }

    /// Stop + delete the optional CPU-sensor service so it isn't left orphaned
    /// pointing at a removed flux.exe. No-op if it was never installed.
    fn remove_sensor_service() {
        const SVC: &str = "FluxSensorService";
        for args in [["stop", SVC], ["delete", SVC]] {
            let _ = Command::new("sc")
                .args(args)
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }

    /// PawnIO's silent uninstall command, if the driver is installed. Used by the
    /// "remove all traces" path since Flux is what installed it for the user.
    fn pawnio_uninstall_string() -> Option<String> {
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO",
                KEY_READ | KEY_WOW64_64KEY,
            )
            .ok()?;
        let s: String = key
            .get_value("QuietUninstallString")
            .or_else(|_| key.get_value("UninstallString"))
            .ok()?;
        if s.trim().is_empty() { None } else { Some(s) }
    }

    /// Remove a previous "fluxid"-branded install (dir, shortcuts, ARP entry) when
    /// upgrading to Flux, so the user isn't left with a duplicate app behind.
    fn remove_legacy_install(scope: Scope) {
        use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, FOLDERID_ProgramFiles};
        let base = match scope {
            Scope::PerUser => known_folder(&FOLDERID_LocalAppData),
            Scope::AllUsers => known_folder(&FOLDERID_ProgramFiles),
        };
        if let Ok(old_dir) = base {
            let old_dir = old_dir.join("fluxid");
            // Best-effort: the old exe was just killed and may still hold a lock.
            for _ in 0..15 {
                if !old_dir.exists() || std::fs::remove_dir_all(&old_dir).is_ok() { break; }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        let root = root_key(scope);
        let _ = root.delete_subkey_all(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\fluxid");
        let _ = root.delete_subkey_all(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Fluxid");
        if let Ok(sm) = start_menu_dir(scope) { let _ = std::fs::remove_file(sm.join("fluxid.lnk")); }
        if let Ok(dt) = desktop_dir(scope) { let _ = std::fs::remove_file(dt.join("fluxid.lnk")); }
    }

    /// Write the widget exe, retrying briefly if it's still locked. A just-killed
    /// widget (live self-update) can hold the file handle for a moment after
    /// taskkill returns, which would otherwise fail the write with a sharing
    /// violation. Retries up to ~6s before giving up.
    fn write_exe_with_retry(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut last = None;
        for attempt in 0..30 {
            match std::fs::write(path, bytes) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    let _ = attempt;
                }
            }
        }
        Err(last.unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "write failed")))
    }

    fn dir_size_kb(dir: &std::path::Path) -> u32 {
        fn walk(dir: &std::path::Path, total: &mut u64) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    if let Ok(ft) = e.file_type() {
                        if ft.is_dir() {
                            walk(&e.path(), total);
                        } else if let Ok(m) = e.metadata() {
                            *total += m.len();
                        }
                    }
                }
            }
        }
        let mut total = 0u64;
        walk(dir, &mut total);
        (total / 1024).min(u32::MAX as u64) as u32
    }

    pub fn install(opts: InstallOptions) -> Result<Report> {
        if !crate::payload::is_bundled() {
            return Err(err(
                "This is a development build with no bundled Flux payload. \
                 Build with scripts/Build-Setup.ps1 to produce an installable exe.",
            ));
        }

        let mut rep = Report::default();
        let dir = install_dir(opts.scope)?;
        let exe = dir.join(EXE_NAME);

        // The CPU-sensor service runs `flux.exe --sensor-service` as LocalSystem,
        // which holds the exe open and would block the overwrite below. Stop it
        // first (we've been relaunched elevated for this when needed — see
        // run_apply_cli), then restart it after the new exe is in place so CPU
        // temperature keeps working. Stopping it also avoids a sharing violation.
        let restart_service = sensor_service_running();
        if restart_service {
            let _ = Command::new("sc")
                .args(["stop", "FluxSensorService"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
            for _ in 0..30 {
                if !sensor_service_running() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        // A running instance (e.g. an upgrade over a live widget) locks the exe.
        kill_running_widget();
        // Upgrading from the old "fluxid" brand: clear out its install so the
        // user ends up with just Flux (config is migrated separately on launch).
        remove_legacy_install(opts.scope);

        std::fs::create_dir_all(&dir)
            .map_err(|e| err(format!("create {}: {e}", dir.display())))?;
        rep.step(format!("Created {}", dir.display()));

        // 1. Write the widget exe. During a live self-update the old widget was
        //    just force-killed, and Windows can take a moment to release its lock
        //    on the exe — so retry briefly instead of failing the whole update.
        write_exe_with_retry(&exe, &crate::payload::flux_exe())
            .map_err(|e| err(format!("write {}: {e}", exe.display())))?;
        rep.step(format!("Installed {EXE_NAME}"));

        // Restart the sensor service we stopped above (now pointing at the new
        // flux.exe), so CPU temperature resumes without waiting for a reboot.
        if restart_service {
            let _ = Command::new("sc")
                .args(["start", "FluxSensorService"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
            rep.step("Restarted sensor service".to_string());
        }

        // 2. Copy ourselves in as the uninstaller.
        let me = std::env::current_exe().map_err(|e| err(format!("current_exe: {e}")))?;
        let uninstaller = dir.join(UNINSTALL_EXE);
        std::fs::copy(&me, &uninstaller)
            .map_err(|e| err(format!("copy uninstaller: {e}")))?;
        rep.step("Installed uninstaller".to_string());

        // Drop the license alongside the app so every user has the terms locally.
        let _ = std::fs::write(dir.join("LICENSE.txt"), include_str!("../../../LICENSE"));

        // 3. Start Menu shortcut (always).
        let sm = start_menu_dir(opts.scope)?;
        std::fs::create_dir_all(&sm).ok();
        let sm_lnk = sm.join(format!("{APP_NAME}.lnk"));
        create_shortcut(&sm_lnk, &exe, &dir, "Flux system monitor")?;
        rep.step("Created Start Menu shortcut".to_string());

        // 4. Desktop shortcut (optional).
        if opts.desktop_shortcut {
            let dt = desktop_dir(opts.scope)?;
            let dt_lnk = dt.join(format!("{APP_NAME}.lnk"));
            create_shortcut(&dt_lnk, &exe, &dir, "Flux system monitor")?;
            rep.step("Created desktop shortcut".to_string());
        }

        // 5. Add/Remove Programs entry.
        let size_kb = dir_size_kb(&dir);
        write_arp_entry(opts.scope, &dir, size_kb)?;
        rep.step("Registered in Add/Remove Programs".to_string());

        // 6. Run at startup (optional, always HKCU / current user).
        if opts.run_at_startup {
            set_run_at_startup(&exe, true)?;
            rep.step("Enabled start with Windows".to_string());
        }

        // Note: the remote-monitoring firewall rule is intentionally NOT added here.
        // Doing it at install time prompted for elevation on EVERY install/update
        // (a scary "Windows Command Processor" UAC), even for users who never use
        // remote monitoring. The widget now adds the rule on demand the first time
        // the feed is enabled (flux-widget/src/firewall.rs), with a UAC that clearly
        // shows "Flux". Uninstall still removes the rule if present.

        // 8. Launch.
        if opts.launch_after {
            launch(opts.scope)?;
            rep.step("Launched Flux".to_string());
        }

        Ok(rep)
    }

    /// Start the installed widget (unelevated when called from the GUI process).
    pub fn launch(scope: Scope) -> Result<()> {
        let dir = install_dir(scope)?;
        Command::new(dir.join(EXE_NAME))
            .current_dir(&dir)
            .spawn()
            .map_err(|e| err(format!("launch Flux: {e}")))?;
        Ok(())
    }

    pub fn uninstall(opts: UninstallOptions) -> Result<Report> {
        let mut rep = Report::default();
        let dir = install_dir(opts.scope)?;
        let exe = dir.join(EXE_NAME);

        kill_running_widget();
        remove_sensor_service();
        rep.step("Stopped Flux".to_string());

        // Shortcuts.
        if let Ok(sm) = start_menu_dir(opts.scope) {
            let _ = std::fs::remove_file(sm.join(format!("{APP_NAME}.lnk")));
        }
        if let Ok(dt) = desktop_dir(opts.scope) {
            let _ = std::fs::remove_file(dt.join(format!("{APP_NAME}.lnk")));
        }
        rep.step("Removed shortcuts".to_string());

        // Startup + ARP registry.
        let _ = set_run_at_startup(&exe, false);
        delete_arp_entry(opts.scope);
        rep.step("Removed registry entries".to_string());

        // Firewall rule (added when the user enabled remote monitoring). Leaving
        // an inbound allow rule behind after uninstall is neither tidy nor safe.
        if remove_firewall_rule() {
            rep.step("Removed firewall rule".to_string());
        }

        // "Remove all traces" (optional): user settings, plus the parts that need
        // elevation — the sensor service, the SYSTEM-written %ProgramData%\Flux
        // data, and the PawnIO driver Flux installed. Batched into one elevated
        // command so it's a single UAC prompt.
        if opts.remove_settings {
            if let Ok(sd) = settings_dir() {
                let _ = std::fs::remove_dir_all(&sd);
                rep.step("Removed user settings".to_string());
            }
            let mut parts: Vec<String> = vec![
                "sc stop FluxSensorService >nul 2>&1".into(),
                "sc delete FluxSensorService >nul 2>&1".into(),
                "rmdir /s /q \"%ProgramData%\\Flux\" >nul 2>&1".into(),
            ];
            if let Some(u) = pawnio_uninstall_string() {
                // QuietUninstallString already carries PawnIO's silent flags.
                parts.push(u);
            }
            let params = format!("/c {}", parts.join(" & "));
            if is_elevated() {
                let _ = Command::new("cmd.exe")
                    .raw_arg(&params)
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
            } else {
                run_elevated_wait(Path::new("cmd.exe"), &params);
            }
            rep.step("Removed sensor service, shared data, and PawnIO driver".to_string());
        }

        // Remove the installed exe now; defer the directory (which still holds
        // the running uninstaller) to a detached cmd that waits for us to exit.
        let _ = std::fs::remove_file(&exe);
        schedule_dir_removal(&dir);
        rep.step("Removed program files".to_string());

        Ok(rep)
    }

    /// Inbound firewall rule the widget adds for remote monitoring. Keep this
    /// name in sync with `flux-widget/src/firewall.rs` (`RULE_NAME`).
    const FIREWALL_RULE: &str = "Flux Remote Sensor";

    /// Remove the remote-monitoring firewall rule if present. Returns true if a
    /// rule existed and a delete was issued. Querying needs no elevation; the
    /// delete does, so we only prompt (one UAC) when a rule actually exists.
    fn remove_firewall_rule() -> bool {
        if !firewall_rule_exists() {
            return false;
        }
        let args = format!(
            "advfirewall firewall delete rule name=\"{FIREWALL_RULE}\""
        );
        if is_elevated() {
            let _ = Command::new("netsh")
                .raw_arg(args)
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        } else {
            // Deleting a firewall rule requires elevation — one UAC prompt.
            run_elevated_wait(Path::new("netsh.exe"), &args);
        }
        true
    }

    /// Create the remote-monitoring inbound rule (delete-then-add for
    /// idempotency). Needs elevation; runs directly when the installer is
    /// already elevated, otherwise one UAC prompt. Returns true if attempted.
    /// No longer called at install time (the widget adds the rule on demand with
    /// a "Flux"-labelled prompt); kept for potential future use.
    #[allow(dead_code)]
    fn add_firewall_rule(port: u16) -> bool {
        let combined = format!(
            "netsh advfirewall firewall delete rule name=\"{FIREWALL_RULE}\" >nul 2>&1 & \
             netsh advfirewall firewall add rule name=\"{FIREWALL_RULE}\" dir=in action=allow \
             protocol=tcp localport={port} profile=private \
             description=\"Flux remote hardware sensor feed\""
        );
        let params = format!("/c {combined}");
        if is_elevated() {
            Command::new("cmd.exe")
                .raw_arg(&params)
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .is_ok()
        } else {
            run_elevated_wait(Path::new("cmd.exe"), &params);
            true
        }
    }

    fn firewall_rule_exists() -> bool {
        Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "show",
                "rule",
                &format!("name={FIREWALL_RULE}"),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Launch `file params` elevated (UAC), hidden, and wait for it to finish.
    fn run_elevated_wait(file: &Path, params: &str) {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};
        use windows::Win32::UI::Shell::{
            ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        };
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let verb = wide("runas");
        let file_w = wide(&file.to_string_lossy());
        let params_w = wide(params);
        unsafe {
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS,
                lpVerb: PCWSTR(verb.as_ptr()),
                lpFile: PCWSTR(file_w.as_ptr()),
                lpParameters: PCWSTR(params_w.as_ptr()),
                nShow: SW_HIDE.0,
                ..Default::default()
            };
            if ShellExecuteExW(&mut info).is_ok() && !info.hProcess.is_invalid() {
                WaitForSingleObject(info.hProcess, INFINITE);
                let _ = CloseHandle(info.hProcess);
            }
        }
    }

    /// Spawn a detached shell that waits a moment (for this process to exit and
    /// release `uninstall.exe`) then deletes the whole install directory.
    ///
    /// The command line is passed with `raw_arg` so `cmd.exe` sees the quoting
    /// verbatim — `Command::arg` would backslash-escape the path's quotes,
    /// which `cmd` doesn't understand, and the `rmdir` would silently no-op.
    fn schedule_dir_removal(dir: &std::path::Path) {
        let _ = Command::new("cmd.exe")
            .raw_arg(format!(
                "/C ping 127.0.0.1 -n 3 >nul & rmdir /S /Q \"{}\"",
                dir.display()
            ))
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();
    }
}

// ──────────────────────────── Non-Windows stubs ────────────────────────────

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn is_elevated() -> bool {
        false
    }
    pub fn sensor_service_running() -> bool {
        false
    }
    pub fn relaunch_elevated_wait(_args: &[String]) -> Result<Option<i32>> {
        Err(err("The Flux installer is Windows-only."))
    }
    pub fn install_dir(_scope: Scope) -> Result<PathBuf> {
        Err(err("The Flux installer is Windows-only."))
    }
    pub fn install(_opts: InstallOptions) -> Result<Report> {
        Err(err("The Flux installer is Windows-only."))
    }
    pub fn uninstall(_opts: UninstallOptions) -> Result<Report> {
        Err(err("The Flux installer is Windows-only."))
    }
    pub fn launch(_scope: Scope) -> Result<()> {
        Err(err("The Flux installer is Windows-only."))
    }
}

pub use imp::{
    install, install_dir, is_elevated, launch, relaunch_elevated_wait, sensor_service_running,
    uninstall,
};
