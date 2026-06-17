//! Accurate CPU temperature via the PawnIO kernel driver (Windows, x86-64).
//!
//! This is the self-contained accurate-temperature backend the user opts into
//! by installing PawnIO from the app (see `flux-widget/src/cpu_driver.rs`).
//! PawnIO only runs cryptographically-signed, sandboxed modules, so we bundle
//! the **officially-signed** modules (LGPL-2.1, from PawnIO.Modules via
//! LibreHardwareMonitor) and load them through `PawnIOLib.dll`:
//!   * `IntelMSR.bin`     — all modern Intel (reads MSRs)
//!   * `AMDFamily17.bin`  — all AMD Zen 1–5 / families 0x17,0x19,0x1A (reads SMN)
//!
//! The decode arithmetic is a faithful port of LibreHardwareMonitor's
//! `IntelCpu.cs` / `Amd17Cpu.cs`. PawnIO itself verifies each module's signature
//! on load, so a tampered module simply fails to load — integrity is guaranteed
//! by the driver, not just by us.
//!
//! This whole path is Windows + x86 only by nature (MSR/SMN). Linux uses hwmon
//! and macOS uses SMC — the CPU-temp source stays platform-abstracted.

// Bundled signed modules. Redistributed unmodified under LGPL-2.1; see
// resources/pawnio/THIRD-PARTY-LICENSES.md.
#[cfg(target_arch = "x86_64")]
const INTEL_MSR_MODULE: &[u8] = include_bytes!("../resources/pawnio/IntelMSR.bin");
#[cfg(target_arch = "x86_64")]
const AMD_FAMILY17_MODULE: &[u8] = include_bytes!("../resources/pawnio/AMDFamily17.bin");

#[cfg(target_arch = "x86_64")]
thread_local! {
    // Outer None = not yet probed; inner None = PawnIO unavailable here.
    static STATE: std::cell::RefCell<Option<Option<imp::PawnIo>>> =
        const { std::cell::RefCell::new(None) };
}

/// Accurate CPU package/die temperature in °C, or `None` if PawnIO isn't
/// installed/loadable or this CPU isn't supported by a bundled module.
#[cfg(target_arch = "x86_64")]
pub fn cpu_temp() -> Option<f32> {
    STATE.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            *guard = Some(imp::PawnIo::open());
        }
        let pio = guard.as_ref().unwrap().as_ref()?;
        pio.read_temp()
    })
}

/// Drop the cached probe so the next `cpu_temp()` re-detects PawnIO. Call after
/// the user installs/removes the driver so the change takes effect without an
/// app restart. Must run on the same (poller) thread that calls `cpu_temp()`.
#[cfg(target_arch = "x86_64")]
pub fn reset() {
    STATE.with(|cell| *cell.borrow_mut() = None);
}

#[cfg(not(target_arch = "x86_64"))]
pub fn cpu_temp() -> Option<f32> {
    None
}

#[cfg(not(target_arch = "x86_64"))]
pub fn reset() {}

#[cfg(target_arch = "x86_64")]
mod imp {
    use super::{AMD_FAMILY17_MODULE, INTEL_MSR_MODULE};
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCSTR;
    use windows::Win32::Foundation::{FreeLibrary, HANDLE, HMODULE};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    // PawnIOLib.dll C ABI — every function is __stdcall and returns an HRESULT.
    type OpenFn = unsafe extern "system" fn(*mut HANDLE) -> i32;
    type LoadFn = unsafe extern "system" fn(HANDLE, *const u8, usize) -> i32;
    type ExecuteFn = unsafe extern "system" fn(
        HANDLE,
        PCSTR,
        *const u64,
        usize,
        *mut u64,
        usize,
        *mut usize,
    ) -> i32;
    type CloseFn = unsafe extern "system" fn(HANDLE) -> i32;

    /// What decode to apply once the matching module is loaded.
    enum Decode {
        /// AMD Zen (family 0x17/0x19/0x1A). `tctl_offset` converts Tctl→Tdie for
        /// the handful of early parts that need it (0 for Zen2+).
        Amd17 { tctl_offset: f32 },
        Intel,
    }

    pub struct PawnIo {
        handle: HANDLE,
        execute: ExecuteFn,
        close: CloseFn,
        lib: HMODULE,
        decode: Decode,
    }

    // The cached state lives in a thread_local and never crosses threads.
    impl Drop for PawnIo {
        fn drop(&mut self) {
            unsafe {
                (self.close)(self.handle);
                let _ = FreeLibrary(self.lib);
            }
        }
    }

    impl PawnIo {
        pub fn open() -> Option<PawnIo> {
            let decode = match cpu_info()? {
                CpuKind::Amd17 => Decode::Amd17 { tctl_offset: amd_tctl_offset() },
                CpuKind::Intel => Decode::Intel,
            };
            let module = match decode {
                Decode::Amd17 { .. } => AMD_FAMILY17_MODULE,
                Decode::Intel => INTEL_MSR_MODULE,
            };

            unsafe {
                let lib = load_pawnio_dll()?;
                let open: OpenFn = std::mem::transmute(GetProcAddress(lib, PCSTR(c"pawnio_open".as_ptr() as *const u8))?);
                let load: LoadFn = std::mem::transmute(GetProcAddress(lib, PCSTR(c"pawnio_load".as_ptr() as *const u8))?);
                let execute: ExecuteFn = std::mem::transmute(GetProcAddress(lib, PCSTR(c"pawnio_execute".as_ptr() as *const u8))?);
                let close: CloseFn = std::mem::transmute(GetProcAddress(lib, PCSTR(c"pawnio_close".as_ptr() as *const u8))?);

                let mut handle = HANDLE::default();
                if open(&mut handle) < 0 || handle.is_invalid() {
                    let _ = FreeLibrary(lib);
                    return None;
                }
                if load(handle, module.as_ptr(), module.len()) < 0 {
                    close(handle);
                    let _ = FreeLibrary(lib);
                    return None;
                }
                Some(PawnIo { handle, execute, close, lib, decode })
            }
        }

        /// Execute a module function with a single-cell in/out (the only shape
        /// the temperature reads need). Returns the first output cell.
        fn exec1(&self, name: PCSTR, input: u64) -> Option<u64> {
            let inp = [input];
            let mut out = [0u64; 1];
            let mut ret_size: usize = 0;
            let hr = unsafe {
                (self.execute)(
                    self.handle,
                    name,
                    inp.as_ptr(),
                    inp.len(),
                    out.as_mut_ptr(),
                    out.len(),
                    &mut ret_size,
                )
            };
            if hr >= 0 && ret_size >= 1 { Some(out[0]) } else { None }
        }

        fn read_smn(&self, addr: u32) -> Option<u32> {
            self.exec1(PCSTR(c"ioctl_read_smn".as_ptr() as *const u8), addr as u64).map(|v| v as u32)
        }

        /// Read an MSR; returns (eax, edx) = (low 32, high 32).
        fn read_msr(&self, msr: u32) -> Option<(u32, u32)> {
            let v = self.exec1(PCSTR(c"ioctl_read_msr".as_ptr() as *const u8), msr as u64)?;
            Some((v as u32, (v >> 32) as u32))
        }

        pub fn read_temp(&self) -> Option<f32> {
            let t = match self.decode {
                Decode::Amd17 { tctl_offset } => self.read_amd17(tctl_offset)?,
                Decode::Intel => self.read_intel()?,
            };
            // Sanity gate: a CPU die is never below ~ -10 or above 130 °C.
            if (0.0..=130.0).contains(&t) { Some(t) } else { None }
        }

        // ── AMD Zen package Tctl/Tdie (port of LHM Amd17Cpu.cs) ──
        fn read_amd17(&self, tctl_offset: f32) -> Option<f32> {
            const F17H_M01H_THM_TCON_CUR_TMP: u32 = 0x0005_9800;
            const RANGE_SEL_MASK: u32 = 0x80000; // bit 19
            const TJ_SEL_MASK: u32 = 0x30000; // bits 17:16
            let raw = self.read_smn(F17H_M01H_THM_TCON_CUR_TMP)?;
            let range_offset = (raw & RANGE_SEL_MASK) != 0 || (raw & TJ_SEL_MASK) == TJ_SEL_MASK;
            let mut t = ((raw >> 21) as f32) * 0.125;
            if range_offset {
                t -= 49.0;
            }
            // tctl_offset is ≤ 0 (Tctl → Tdie); 0 for Zen2+ incl. Ryzen 9000.
            Some(t + tctl_offset)
        }

        // ── Intel package temperature (port of LHM IntelCpu.cs) ──
        fn read_intel(&self) -> Option<f32> {
            const IA32_TEMPERATURE_TARGET: u32 = 0x01A2;
            const IA32_PACKAGE_THERM_STATUS: u32 = 0x01B1;
            const IA32_THERM_STATUS: u32 = 0x019C;

            let tjmax = match self.read_msr(IA32_TEMPERATURE_TARGET) {
                Some((eax, _)) if ((eax >> 16) & 0xFF) > 0 => ((eax >> 16) & 0xFF) as f32,
                _ => 100.0,
            };
            // Prefer the package thermal status; fall back to this core's status.
            let read_delta = |msr: u32| -> Option<f32> {
                let (eax, _) = self.read_msr(msr)?;
                if eax & 0x8000_0000 == 0 {
                    return None; // reading-valid bit not set
                }
                Some(((eax & 0x007F_0000) >> 16) as f32)
            };
            let delta = read_delta(IA32_PACKAGE_THERM_STATUS)
                .or_else(|| read_delta(IA32_THERM_STATUS))?;
            Some(tjmax - delta)
        }
    }

    /// Which bundled module (if any) matches this CPU.
    enum CpuKind {
        Amd17,
        Intel,
    }

    fn cpu_info() -> Option<CpuKind> {
        use std::arch::x86_64::__cpuid;
        let v = __cpuid(0);
        let vendor = vendor_string(v.ebx, v.edx, v.ecx);
        let f = __cpuid(1);
        let eax = f.eax;
        let base_family = (eax >> 8) & 0xF;
        let ext_family = (eax >> 20) & 0xFF;
        let family = if base_family == 0xF { base_family + ext_family } else { base_family };
        match vendor.as_str() {
            // Zen 1–5 (and beyond) all use the Family17h SMN thermal register.
            "AuthenticAMD" if matches!(family, 0x17 | 0x19 | 0x1A) => Some(CpuKind::Amd17),
            "GenuineIntel" => Some(CpuKind::Intel),
            _ => None,
        }
    }

    fn vendor_string(ebx: u32, edx: u32, ecx: u32) -> String {
        let mut b = Vec::with_capacity(12);
        b.extend_from_slice(&ebx.to_le_bytes());
        b.extend_from_slice(&edx.to_le_bytes());
        b.extend_from_slice(&ecx.to_le_bytes());
        String::from_utf8_lossy(&b).to_string()
    }

    /// Tctl→Tdie offset for the few early Ryzen/Threadripper parts that need it
    /// (mirrors LHM's name table; 0 for Zen2+). Matched on the CPUID brand string.
    fn amd_tctl_offset() -> f32 {
        let brand = brand_string();
        if brand.contains("1600X") || brand.contains("1700X") || brand.contains("1800X") {
            -20.0
        } else if brand.contains("Threadripper") && (brand.contains(" 19") || brand.contains(" 29")) {
            -27.0
        } else if brand.contains("2700X") {
            -10.0
        } else {
            0.0
        }
    }

    fn brand_string() -> String {
        use std::arch::x86_64::__cpuid;
        if __cpuid(0x8000_0000).eax < 0x8000_0004 {
            return String::new();
        }
        let mut bytes = Vec::with_capacity(48);
        for leaf in [0x8000_0002u32, 0x8000_0003, 0x8000_0004] {
            let r = __cpuid(leaf);
            for reg in [r.eax, r.ebx, r.ecx, r.edx] {
                bytes.extend_from_slice(&reg.to_le_bytes());
            }
        }
        String::from_utf8_lossy(&bytes).trim_end_matches('\0').trim().to_string()
    }

    /// Load `PawnIOLib.dll` — preferring the install dir recorded in the
    /// registry, falling back to the loader search path.
    unsafe fn load_pawnio_dll() -> Option<HMODULE> {
        if let Some(dir) = install_location() {
            let mut path = std::path::PathBuf::from(dir);
            path.push("PawnIOLib.dll");
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            if let Ok(h) = LoadLibraryW(windows::core::PCWSTR(wide.as_ptr())) {
                if !h.is_invalid() {
                    return Some(h);
                }
            }
        }
        let fallback: Vec<u16> = "PawnIOLib.dll".encode_utf16().chain(std::iter::once(0)).collect();
        match LoadLibraryW(windows::core::PCWSTR(fallback.as_ptr())) {
            Ok(h) if !h.is_invalid() => Some(h),
            _ => None,
        }
    }

    fn install_location() -> Option<String> {
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO",
                KEY_READ | KEY_WOW64_64KEY,
            )
            .ok()?;
        let loc: String = key.get_value("InstallLocation").ok()?;
        if loc.is_empty() { None } else { Some(loc) }
    }
}
