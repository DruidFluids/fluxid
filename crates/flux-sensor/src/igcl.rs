#![cfg(windows)]
#![allow(dead_code)] // layout structs documented for offsets; reads use raw bytes
//! Optional Intel GPU telemetry via IGCL (Intel Graphics Control Library,
//! `ControlLib.dll`) — reads the live GPU **core clock** on Intel parts, where
//! D3DKMT reports 0 Hz at idle (so the tile would otherwise show a bare "—").
//!
//! Fully optional and dynamically loaded: the DLL is absent on non-Intel systems
//! (load fails → no-op). It's **self-tuning** — rather than hardcode IGCL's struct
//! version / size (which vary by driver), it sweeps the `ctlInit` AppVersion and
//! the telemetry `Size` field until the driver accepts them, caches the working
//! combo, and reads the clock at a fixed offset. Every value is sanity-ranged and
//! any failure falls back to the D3DKMT node-scan, so it can't show garbage or
//! destabilise other vendors.

use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::ptr::null_mut;
use windows::core::{s, w};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type ApiHandle = *mut c_void;
type DeviceHandle = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct AppId {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

// `ctl_version_info_t` is a scalar `typedef uint32_t` (NOT a {u16,u16} struct) —
// `CTL_MAKE_VERSION(major, minor) = (major << 16) | (minor & 0xffff)`. Getting this
// wrong (a 2-byte-aligned struct) pushed AppVersion from its real offset 8 to 6, so
// strict drivers read a garbage major version and reject ctlInit with
// UNSUPPORTED_VERSION even when the requested version is supported.
fn make_version(major: u16, minor: u16) -> u32 {
    ((major as u32) << 16) | (minor as u32)
}
fn version_parts(v: u32) -> (u16, u16) {
    ((v >> 16) as u16, (v & 0xffff) as u16)
}

#[repr(C)]
struct InitArgs {
    size: u32,
    version: u8,
    app_version: u32,       // ctl_version_info_t (uint32_t @ offset 8, 4-aligned)
    flags: u32,
    supported_version: u32,
    application_uid: AppId,
}

// ctl_oc_telemetry_item_t: { bool bSupported; ctl_units_t units; ctl_data_type_t
// type; ctl_data_value_t value; } → 24 bytes (the f64 value forces 8-byte align):
// b_supported@0, units@4, data_type@8, value@16.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct TelemetryItem {
    b_supported: u8,
    units: i32,
    data_type: i32,
    value: f64,
}

type FnInit = unsafe extern "system" fn(*mut InitArgs, *mut ApiHandle) -> i32;
type FnEnumerate = unsafe extern "system" fn(ApiHandle, *mut u32, *mut DeviceHandle) -> i32;
type FnTelemetry = unsafe extern "system" fn(DeviceHandle, *mut c_void) -> i32;
type FnClose = unsafe extern "system" fn(ApiHandle) -> i32;

// Byte offsets within ctl_power_telemetry_t: header (Size u32 @0, Version u8 @4)
// then 8-aligned 24-byte telemetry items from offset 8. The item order is
// timeStamp, gpuEnergyCounter, gpuVoltage, gpuCurrentClockFrequency,
// gpuCurrentTemperature, … so the clock is item 3 (8 + 3*24 = 80) and the
// temperature the next item (8 + 4*24 = 104). Each item: b_supported@0, units@4,
// data_type@8, value(f64)@16.
const CLOCK_ITEM_OFFSET: usize = 80;
const CLOCK_SUPPORTED_OFFSET: usize = CLOCK_ITEM_OFFSET;
const CLOCK_UNITS_OFFSET: usize = CLOCK_ITEM_OFFSET + 4;
const CLOCK_VALUE_OFFSET: usize = CLOCK_ITEM_OFFSET + 16;
const TEMP_ITEM_OFFSET: usize = 104;
const TEMP_SUPPORTED_OFFSET: usize = TEMP_ITEM_OFFSET;
const TEMP_UNITS_OFFSET: usize = TEMP_ITEM_OFFSET + 4;
const TEMP_VALUE_OFFSET: usize = TEMP_ITEM_OFFSET + 16;

/// ApplicationUIDs to try, in order. A driver may want the zeroed UID (Intel's
/// own samples pass zero) or accept any nonzero one; a strict newer driver rejects
/// an unregistered nonzero UID with UNKNOWN_APPLICATION_UID. Verified on a
/// Server-2022 iGPU: the zeroed UID is accepted, the nonzero one is not — so try
/// zero first, then the nonzero fallback for older drivers. (The earlier belief
/// that all-zero was rejected was actually the AppVersion struct-layout bug.)
const ZERO_UID: AppId = AppId { d1: 0, d2: 0, d3: 0, d4: [0; 8] };
const APP_UID: AppId = AppId {
    d1: 0xF100_D000,
    d2: 0x4C5F,
    d3: 0x11EE,
    d4: [0xB9, 0x62, 0x02, 0x42, 0xAC, 0x12, 0x00, 0x02],
};
const APP_UIDS: &[AppId] = &[ZERO_UID, APP_UID];

/// AppVersions to try for `ctlInit`, in order. v1.1 is the current impl version
/// (CTL_IMPL_MAJOR/MINOR_VERSION) and is accepted first on the tested driver; the
/// rest are a small fallback matrix for other driver revisions.
const APP_VERSIONS: &[(u16, u16)] = &[(1, 1), (1, 2), (1, 3), (2, 0), (1, 0)];

/// Telemetry buffer: generously larger than any plausible ctl_power_telemetry_t,
/// 8-aligned (Vec<u64>), so a Size sweep can never overrun it.
const BUF_U64: usize = 512; // 4096 bytes
const SIZE_SWEEP: std::ops::Range<u32> = 256..1400;

struct Igcl {
    _hmod: HMODULE,
    close: FnClose,
    telemetry: FnTelemetry,
    api: ApiHandle,
    devices: Vec<DeviceHandle>,
    tele_size: u32,
}

thread_local! {
    static CACHE: RefCell<Cache> = const { RefCell::new(Cache::Uninit) };
}

enum Cache {
    Uninit,
    Failed,
    Ready(Igcl),
}

unsafe fn resolve(hmod: HMODULE) -> Option<(FnInit, FnEnumerate, FnTelemetry, FnClose)> {
    let init: FnInit = std::mem::transmute(GetProcAddress(hmod, s!("ctlInit"))?);
    let enumerate: FnEnumerate = std::mem::transmute(GetProcAddress(hmod, s!("ctlEnumerateDevices"))?);
    let telemetry: FnTelemetry = std::mem::transmute(GetProcAddress(hmod, s!("ctlPowerTelemetryGet"))?);
    let close: FnClose = std::mem::transmute(GetProcAddress(hmod, s!("ctlClose"))?);
    Some((init, enumerate, telemetry, close))
}

/// Sweep ApplicationUID × AppVersion until `ctlInit` succeeds. Returns the api
/// handle + the (major,minor) that worked.
unsafe fn try_init(init: FnInit) -> Option<(ApiHandle, (u16, u16))> {
    for &uid in APP_UIDS {
      for &(major, minor) in APP_VERSIONS {
        let mut args = InitArgs {
            size: std::mem::size_of::<InitArgs>() as u32,
            version: 0,
            app_version: make_version(major, minor),
            flags: 0,
            supported_version: 0,
            application_uid: uid,
        };
        let mut api: ApiHandle = null_mut();
        if init(&mut args, &mut api) == 0 && !api.is_null() {
            return Some((api, (major, minor)));
        }
      }
    }
    None
}

/// Sweep the telemetry Size on each device until one is accepted. Returns the
/// working (Size, device index).
unsafe fn find_tele_size(telemetry: FnTelemetry, devices: &[DeviceHandle]) -> Option<(u32, usize)> {
    let mut buf = vec![0u64; BUF_U64];
    let base = buf.as_mut_ptr() as *mut u8;
    for (idx, &dev) in devices.iter().enumerate() {
        let mut size = SIZE_SWEEP.start;
        while size < SIZE_SWEEP.end {
            std::ptr::write_bytes(base, 0, BUF_U64 * 8);
            *(base as *mut u32) = size;
            *base.add(4) = 1;
            if telemetry(dev, base as *mut c_void) == 0 {
                return Some((size, idx));
            }
            size += 4;
        }
    }
    None
}

unsafe fn discover() -> Option<Igcl> {
    let hmod = LoadLibraryW(w!("ControlLib.dll")).ok().filter(|h| !h.is_invalid())?;
    let Some((init, enumerate, telemetry, close)) = resolve(hmod) else {
        let _ = FreeLibrary(hmod);
        return None;
    };
    let Some((api, _ver)) = try_init(init) else {
        let _ = FreeLibrary(hmod);
        return None;
    };
    // Enumerate devices (two-call: count, then fill).
    let mut count: u32 = 0;
    if enumerate(api, &mut count, null_mut()) != 0 || count == 0 {
        close(api);
        let _ = FreeLibrary(hmod);
        return None;
    }
    let mut devices: Vec<DeviceHandle> = vec![null_mut(); count as usize];
    if enumerate(api, &mut count, devices.as_mut_ptr()) != 0 {
        close(api);
        let _ = FreeLibrary(hmod);
        return None;
    }
    let Some((tele_size, _)) = find_tele_size(telemetry, &devices) else {
        close(api);
        let _ = FreeLibrary(hmod);
        return None;
    };
    Some(Igcl { _hmod: hmod, close, telemetry, api, devices, tele_size })
}

impl Igcl {
    /// Read the highest plausible GPU core clock (MHz) and temperature (°C) across
    /// devices from a single telemetry fetch per device. Each value is sanity-gated
    /// independently, so a wrong struct offset for one yields `None` for it rather
    /// than a bogus reading — and a part that reports a clock but no temp (or vice
    /// versa) still returns whatever it does expose.
    unsafe fn read_clock_temp(&self) -> (Option<f32>, Option<f32>) {
        let mut buf = vec![0u64; BUF_U64];
        let base = buf.as_mut_ptr() as *mut u8;
        let mut best_clock = 0.0f32;
        let mut best_temp = 0.0f32;
        for &dev in &self.devices {
            std::ptr::write_bytes(base, 0, BUF_U64 * 8);
            *(base as *mut u32) = self.tele_size;
            *base.add(4) = 1;
            if (self.telemetry)(dev, base as *mut c_void) == 0 {
                if *base.add(CLOCK_SUPPORTED_OFFSET) != 0 {
                    let v = *(base.add(CLOCK_VALUE_OFFSET) as *const f64);
                    if (1.0..6000.0).contains(&v) {
                        best_clock = best_clock.max(v as f32);
                    }
                }
                if *base.add(TEMP_SUPPORTED_OFFSET) != 0 {
                    let v = *(base.add(TEMP_VALUE_OFFSET) as *const f64);
                    if (1.0..130.0).contains(&v) {
                        best_temp = best_temp.max(v as f32);
                    }
                }
            }
        }
        ((best_clock > 0.0).then_some(best_clock), (best_temp > 0.0).then_some(best_temp))
    }
}

/// Live GPU core clock (MHz) and temperature (°C) via IGCL — either `None` if IGCL
/// is unavailable / the read fails / the value is implausible. Initialises once per
/// thread and caches. The temperature only populates on Intel parts/drivers that
/// expose `gpuCurrentTemperature` (D3DKMT reports no separate iGPU temp on many of
/// them); both are independently sanity-gated.
pub fn gpu_clock_temp() -> (Option<f32>, Option<f32>) {
    CACHE.with(|cell| {
        let mut c = cell.borrow_mut();
        if matches!(&*c, Cache::Uninit) {
            *c = match unsafe { discover() } {
                Some(i) => Cache::Ready(i),
                None => Cache::Failed,
            };
        }
        match &*c {
            Cache::Ready(igcl) => unsafe { igcl.read_clock_temp() },
            _ => (None, None),
        }
    })
}

/// Verbose IGCL diagnostic for `--gpu-debug`: reports DLL load, the working init
/// AppVersion, device count, telemetry Size, and the clock item (supported/units/
/// value) — so the live read can be cross-checked against reality.
pub fn probe() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "\n-- IGCL (Intel control library) --");
    unsafe { probe_inner(&mut s) };
    s
}

/// Decode the IGCL `ctl_result_t` codes seen in the field into short names, so the
/// `--gpu-debug` report is readable without an IGCL header on hand.
fn result_name(r: i32) -> &'static str {
    match r as u32 {
        0x0000_0000 => "SUCCESS",
        0x4000_0001 => "NOT_INITIALIZED",
        0x4000_0002 => "ALREADY_INITIALIZED",
        0x4000_0007 => "NOT_AVAILABLE",
        0x4000_0009 => "UNSUPPORTED_VERSION",
        0x4000_000C => "INVALID_API_HANDLE",
        0x4000_0020 => "PLATFORM_NOT_SUPPORTED",
        0x4000_0021 => "UNKNOWN_APPLICATION_UID",
        _ => "",
    }
}

unsafe fn probe_inner(s: &mut String) {
    let hmod = match LoadLibraryW(w!("ControlLib.dll")) {
        Ok(h) if !h.is_invalid() => h,
        _ => {
            let _ = writeln!(s, "  ControlLib.dll: not found (no Intel control runtime — expected on non-Intel)");
            return;
        }
    };
    let _ = writeln!(s, "  ControlLib.dll: loaded");
    let Some((init, enumerate, telemetry, close)) = resolve(hmod) else {
        let _ = writeln!(s, "  exports: missing one of ctlInit/Enumerate/Telemetry/Close");
        let _ = FreeLibrary(hmod);
        return;
    };

    // Instrumented init sweep — report each attempt's decoded result code and the
    // SupportedVersion IGCL echoes back, so a failed init can be diagnosed across
    // machines rather than guessed. Try both the zeroed UID (what Intel's own
    // samples pass) and our nonzero UID across the version matrix.
    //
    // Observed on a Server-2022 iGPU + newer driver: nonzero UID → 0x40000021
    // UNKNOWN_APPLICATION_UID; zeroed UID → 0x40000009 UNSUPPORTED_VERSION. Newer
    // drivers validate the UID against Intel's registered list (older ones, e.g. a
    // working laptop, accept any nonzero UID). Telemetry is ONLY attempted after a
    // clean (result==0) init — pushing a half-initialised handle into the Size
    // sweep blocks the driver call indefinitely.
    let zero_uid = AppId { d1: 0, d2: 0, d3: 0, d4: [0; 8] };
    let uids: &[(&str, AppId)] = &[("zero", zero_uid), ("nonzero", APP_UID)];
    let mut api: ApiHandle = null_mut();
    let mut ok_label: Option<String> = None;
    'sweep: for &(uid_name, uid) in uids {
        for &(major, minor) in APP_VERSIONS {
            let mut args = InitArgs {
                size: std::mem::size_of::<InitArgs>() as u32,
                version: 0,
                app_version: make_version(major, minor),
                flags: 0,
                supported_version: 0,
                application_uid: uid,
            };
            let mut h: ApiHandle = null_mut();
            let r = init(&mut args, &mut h);
            let (smaj, smin) = version_parts(args.supported_version);
            let _ = writeln!(
                s,
                "  ctlInit uid={uid_name} v{major}.{minor}: result=0x{r:08X} {}  supported=v{smaj}.{smin}  handle={}",
                result_name(r),
                if h.is_null() { "null" } else { "set" },
            );
            if r == 0 && !h.is_null() {
                api = h;
                ok_label = Some(format!("uid={uid_name} v{major}.{minor}"));
                break 'sweep;
            }
        }
    }
    let Some(label) = ok_label else {
        let _ = writeln!(s, "  ctlInit: no AppVersion/UID combination succeeded — IGCL telemetry unavailable on this driver");
        let _ = FreeLibrary(hmod);
        return;
    };
    let _ = writeln!(s, "  ctlInit: OK with {label}");

    let mut count: u32 = 0;
    let r = enumerate(api, &mut count, null_mut());
    let _ = writeln!(s, "  ctlEnumerateDevices: result=0x{r:08X} {}  count={count}", result_name(r));
    if r == 0 && count > 0 {
        let mut devices: Vec<DeviceHandle> = vec![null_mut(); count as usize];
        if enumerate(api, &mut count, devices.as_mut_ptr()) == 0 {
            match find_tele_size(telemetry, &devices) {
                Some((size, idx)) => {
                    let mut buf = vec![0u64; BUF_U64];
                    let base = buf.as_mut_ptr() as *mut u8;
                    *(base as *mut u32) = size;
                    *base.add(4) = 1;
                    let _ = telemetry(devices[idx], base as *mut c_void);
                    let c_sup = *base.add(CLOCK_SUPPORTED_OFFSET) != 0;
                    let c_units = *(base.add(CLOCK_UNITS_OFFSET) as *const i32);
                    let c_val = *(base.add(CLOCK_VALUE_OFFSET) as *const f64);
                    let t_sup = *base.add(TEMP_SUPPORTED_OFFSET) != 0;
                    let t_units = *(base.add(TEMP_UNITS_OFFSET) as *const i32);
                    let t_val = *(base.add(TEMP_VALUE_OFFSET) as *const f64);
                    let _ = writeln!(
                        s,
                        "  telemetry OK: device {idx} Size={size}\n    clock: supported={c_sup} units={c_units} value={c_val:.1}\n    temp:  supported={t_sup} units={t_units} value={t_val:.1}",
                    );
                }
                None => {
                    let _ = writeln!(s, "  ctlPowerTelemetryGet: no Size in {}..{} accepted", SIZE_SWEEP.start, SIZE_SWEEP.end);
                }
            }
        }
    }
    close(api);
    let _ = FreeLibrary(hmod);
}
