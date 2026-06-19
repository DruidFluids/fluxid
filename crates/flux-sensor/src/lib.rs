//! Cross-platform sensor polling (CPU/GPU/RAM/disk/network) via sysinfo plus
//! vendor APIs (DXGI/NVML on Windows).

use flux_core::sensor_data::*;
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use sysinfo::{Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System};
use std::time::{SystemTime, UNIX_EPOCH};

// Optional accurate CPU temperature via the user-installed PawnIO driver.
#[cfg(windows)]
mod pawnio;
#[cfg(windows)]
mod d3dkmt;

/// Drop the cached CPU-temp driver probe so the next poll re-detects PawnIO.
/// Call right after the user installs or removes the driver. Must be called on
/// the same thread that polls sensors.
pub fn refresh_cpu_temp_driver() {
    #[cfg(windows)]
    pawnio::reset();
}

/// Read CPU die temperature directly from PawnIO. This requires an **elevated**
/// process, so it's used by the Flux sensor service (which publishes the value
/// for the non-elevated widget via `flux_core::sensor_ipc`). Returns `None` off
/// Windows or when PawnIO is unavailable/inaccessible.
pub fn privileged_cpu_temp() -> Option<f32> {
    #[cfg(windows)]
    {
        pawnio::cpu_temp()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

// ── GPU sources ──────────────────────────────────────────────────────────────
//
// The GPU tile is fed by one per-OS/vendor backend behind the `GpuSource` trait,
// so a new platform (Linux DRM, macOS IOReport, …) slots in by adding a struct +
// one arm in `select_gpu_source` — `read_gpu` never changes. Each backend owns
// its own handle/state (NVML device, D3DKMT adapter LUID + usage sampler) so it
// doesn't re-discover the adapter on the hot path.

/// Static GPU identity, resolved once at startup (it doesn't change mid-run).
#[derive(Debug, Clone, Default)]
struct GpuIdentity {
    name: String,
    is_integrated: bool,
}

/// Live, per-poll GPU metrics. Fields a backend can't supply stay `None`/0.0 so
/// the tile degrades to an em-dash, exactly as before.
#[derive(Debug, Clone, Default)]
struct GpuMetrics {
    usage_percent: f32,
    temperature_c: Option<f32>,
    clock_mhz: Option<f32>,
    vram_used_mb: f32,
    vram_total_mb: f32,
    fan_rpm: Option<f32>,
}

trait GpuSource: Send {
    /// Current live metrics. Called every poll; cheap, non-blocking, never panics.
    fn read(&mut self) -> GpuMetrics;
    /// Static identity (name + integrated flag). Read once at startup.
    fn identity(&self) -> GpuIdentity;
    /// Short label for the startup log.
    fn kind(&self) -> &'static str;
}

/// NVIDIA via NVML — full data; cross-platform (Windows + Linux). Never integrated.
#[cfg(any(windows, target_os = "linux"))]
struct NvmlGpu {
    nvml: Nvml,
}
#[cfg(any(windows, target_os = "linux"))]
impl GpuSource for NvmlGpu {
    fn read(&mut self) -> GpuMetrics {
        let Ok(d) = self.nvml.device_by_index(0) else { return GpuMetrics::default() };
        let (vram_used, vram_total) = d
            .memory_info()
            .map(|m| (m.used as f32 / 1_048_576.0, m.total as f32 / 1_048_576.0))
            .unwrap_or((0.0, 0.0));
        GpuMetrics {
            usage_percent: d.utilization_rates().map(|u| u.gpu as f32).unwrap_or(0.0),
            temperature_c: d.temperature(TemperatureSensor::Gpu).ok().map(|t| t as f32),
            clock_mhz: d
                .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
                .ok()
                .map(|c| c as f32),
            vram_used_mb: vram_used,
            vram_total_mb: vram_total,
            fan_rpm: None,
        }
    }
    fn identity(&self) -> GpuIdentity {
        let name = self
            .nvml
            .device_by_index(0)
            .ok()
            .and_then(|d| d.name().ok())
            .map(|n| n.replace("NVIDIA ", ""))
            .unwrap_or_else(|| "GPU".into());
        GpuIdentity { name, is_integrated: false }
    }
    fn kind(&self) -> &'static str { "NVML (NVIDIA)" }
}

/// AMD / Intel / unrecognized on Windows: DXGI for name + VRAM, D3DKMT for the
/// live clock / usage / temperature DXGI itself doesn't expose.
#[cfg(windows)]
struct DxgiGpu {
    luid: u64,
    usage: d3dkmt::UsageSampler,
}
#[cfg(windows)]
impl GpuSource for DxgiGpu {
    fn read(&mut self) -> GpuMetrics {
        let (used, total) = dxgi_query().map(|(_, u, t, _)| (u, t)).unwrap_or((0.0, 0.0));
        let luid = windows::Win32::Foundation::LUID {
            LowPart: self.luid as u32,
            HighPart: (self.luid >> 32) as i32,
        };
        // read_clock_temp gives Some(0.0) when the clock query succeeds but the
        // engine is idle/clock-gated, and None only when the query is unsupported —
        // so the tile can keep the row reserved (showing "—" at idle) yet stay
        // hidden on GPUs that genuinely report no clock.
        let (clock_mhz, temperature_c) = d3dkmt::read_clock_temp(luid);
        GpuMetrics {
            usage_percent: self.usage.read(luid).unwrap_or(0.0),
            temperature_c,
            clock_mhz,
            vram_used_mb: used,
            vram_total_mb: total,
            fan_rpm: None,
        }
    }
    fn identity(&self) -> GpuIdentity {
        let name = dxgi_query().map(|(n, ..)| n).unwrap_or_else(|| "GPU".into());
        let is_integrated = gpu_name_is_integrated(&name);
        GpuIdentity { name, is_integrated }
    }
    fn kind(&self) -> &'static str { "DXGI + D3DKMT (AMD/Intel)" }
}

/// Apple Silicon (macOS). Always integrated (unified memory).
#[cfg(target_os = "macos")]
struct AppleGpu;
#[cfg(target_os = "macos")]
impl GpuSource for AppleGpu {
    fn read(&mut self) -> GpuMetrics {
        match apple_gpu_query() {
            Some((_, used, total, temp)) => GpuMetrics {
                temperature_c: temp,
                vram_used_mb: used,
                vram_total_mb: total,
                ..Default::default()
            },
            None => GpuMetrics::default(),
        }
    }
    fn identity(&self) -> GpuIdentity {
        let name = apple_gpu_query().map(|(n, ..)| n).unwrap_or_else(|| "GPU".into());
        GpuIdentity { name, is_integrated: true }
    }
    fn kind(&self) -> &'static str { "Apple (Metal/IOKit)" }
}

/// Last resort: no live metrics. `read_gpu` still adds a components temperature.
struct NoneGpu;
impl GpuSource for NoneGpu {
    fn read(&mut self) -> GpuMetrics { GpuMetrics::default() }
    fn identity(&self) -> GpuIdentity { GpuIdentity { name: "GPU".into(), is_integrated: false } }
    fn kind(&self) -> &'static str { "sysinfo components (temp only)" }
}

/// Pick the best available backend for this machine, once at startup.
fn select_gpu_source() -> Box<dyn GpuSource> {
    #[cfg(any(windows, target_os = "linux"))]
    {
        if let Ok(nvml) = Nvml::init() {
            if nvml.device_by_index(0).is_ok() {
                return Box::new(NvmlGpu { nvml });
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if apple_gpu_query().is_some() {
            return Box::new(AppleGpu);
        }
    }
    #[cfg(windows)]
    {
        if let Some((.., luid)) = dxgi_query() {
            return Box::new(DxgiGpu { luid, usage: d3dkmt::UsageSampler::default() });
        }
    }
    Box::new(NoneGpu)
}

pub struct SensorPoller {
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    // Static GPU identity (name + integrated), resolved once at startup.
    gpu_identity: GpuIdentity,
    // Per-OS live-metrics backend (NVML / DXGI+D3DKMT / Apple / none).
    gpu_source: Box<dyn GpuSource>,
    // Persistent PDH query for the live CPU clock (Windows boost-aware).
    cpu_freq: Option<CpuFreq>,
}

// ─────────────────────────────────────────────────────────────────────────
//  Live CPU clock via PDH (Windows) — matches Task Manager's "Speed".
//  sysinfo's frequency() returns the static base MHz; we scale it by
//  `\Processor Information(_Total)\% Processor Performance` (can exceed 100%
//  under boost) to get the effective clock. A persistent query avoids
//  re-opening PDH every poll. On non-Windows this is an inert stub so the
//  rest of read_cpu compiles and just uses the base frequency.
// ─────────────────────────────────────────────────────────────────────────
#[cfg(windows)]
struct CpuFreq {
    query: windows::Win32::System::Performance::PDH_HQUERY,
    counter: windows::Win32::System::Performance::PDH_HCOUNTER,
}

#[cfg(windows)]
impl CpuFreq {
    fn new() -> Option<Self> {
        use windows::Win32::System::Performance::{
            PdhAddEnglishCounterW, PdhCollectQueryData, PdhOpenQueryW, PDH_HCOUNTER, PDH_HQUERY,
        };
        use windows::core::w;

        let mut query = PDH_HQUERY::default();
        // PdhOpenQueryW(szDataSource: PCWSTR, dwUserData: usize, phQuery: *mut PDH_HQUERY) -> u32
        let status = unsafe { PdhOpenQueryW(None, 0, &mut query) };
        if status != 0 {
            return None;
        }

        let mut counter = PDH_HCOUNTER::default();
        // PdhAddEnglishCounterW(hQuery, szFullCounterPath: PCWSTR, dwUserData, phCounter: *mut PDH_HCOUNTER) -> u32
        let status = unsafe {
            PdhAddEnglishCounterW(
                query,
                w!("\\Processor Information(_Total)\\% Processor Performance"),
                0,
                &mut counter,
            )
        };
        if status != 0 {
            return None;
        }

        // Precision counters need two collects before a value is valid; prime once.
        unsafe { PdhCollectQueryData(query) };

        Some(Self { query, counter })
    }

    /// Effective-clock multiplier (e.g. 1.10 = 110% of base under boost).
    fn current_ratio(&mut self) -> Option<f32> {
        use windows::Win32::System::Performance::{
            PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
        };

        if unsafe { PdhCollectQueryData(self.query) } != 0 {
            return None;
        }

        let mut value = PDH_FMT_COUNTERVALUE::default();
        // PdhGetFormattedCounterValue(hCounter, dwFormat: PDH_FMT, lpdwType: Option<*mut u32>, pValue: *mut PDH_FMT_COUNTERVALUE) -> u32
        let status = unsafe {
            PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, None, &mut value)
        };
        if status != 0 {
            return None;
        }

        let pct = unsafe { value.Anonymous.doubleValue };
        if pct.is_finite() && pct > 0.0 {
            Some((pct / 100.0) as f32)
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
struct CpuFreq;

#[cfg(not(windows))]
impl CpuFreq {
    fn new() -> Option<Self> {
        None
    }
    fn current_ratio(&mut self) -> Option<f32> {
        None
    }
}

fn shorten_cpu_name(name: &str) -> String {
    let mut n = name.replace("(R)", "").replace("(TM)", "");
    if let Some(idx) = n.find("-Core Processor") {
        if let Some(sp) = n[..idx].rfind(' ') {
            n.truncate(sp);
        }
    }
    n.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ─────────────────────────────────────────────────────────────────────────
//  GPU: DXGI fallback (AMD / Intel / unrecognized Windows GPUs)
//  Gives name + VRAM used/total via IDXGIFactory1::EnumAdapters1 and
//  IDXGIAdapter3::QueryVideoMemoryInfo. Temp/clock/load are not available
//  through DXGI and degrade to None (em-dash in tiles).
// ─────────────────────────────────────────────────────────────────────────
// Returns (name, vram_used_mb, vram_total_mb, adapter_luid). The LUID is packed
// (HighPart<<32 | LowPart) so the signature stays platform-neutral; on Windows we
// unpack it to feed D3DKMT for live clock/usage/temp.
#[cfg(windows)]
fn dxgi_query() -> Option<(String, f32, f32, u64)> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_ADAPTER_DESC1,
        DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
    };
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut best: Option<(String, f32, f32, u64)> = None;
        let mut best_dedicated = -1.0f32;
        let mut i = 0u32;
        loop {
            let adapter = match factory.EnumAdapters1(i) {
                Ok(a) => a,
                Err(_) => break,
            };
            i += 1;
            let desc: DXGI_ADAPTER_DESC1 = match adapter.GetDesc1() {
                Ok(d) => d,
                Err(_) => continue,
            };
            // Skip the Microsoft Basic Render (software) adapter
            if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                continue;
            }
            let raw = String::from_utf16_lossy(&desc.Description);
            let name = raw.trim_end_matches('\0').trim().to_string();
            let dedicated_mb = desc.DedicatedVideoMemory as f32 / (1024.0 * 1024.0);
            let shared_mb = desc.SharedSystemMemory as f32 / (1024.0 * 1024.0);
            // A small-carve-out iGPU (e.g. Intel Xe/Arc reports only ~128 MB
            // "dedicated") really runs on shared system RAM, so its dedicated figure
            // is useless (that's what produced "0.2/0.1 GB"). Detect that and report
            // the shared budget + usage across BOTH memory segments. A large-UMA
            // iGPU (e.g. 780M with a 4 GB BIOS carve-out) and discrete GPUs keep
            // their dedicated figure.
            let small_carveout = dedicated_mb < 512.0;
            let total_mb = if small_carveout { dedicated_mb + shared_mb } else { dedicated_mb };
            let luid = ((desc.AdapterLuid.HighPart as u64) << 32)
                | (desc.AdapterLuid.LowPart as u64);
            let mut used_mb = 0.0f32;
            if let Ok(adapter3) = adapter.cast::<IDXGIAdapter3>() {
                let usage = |grp| -> f32 {
                    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                    if adapter3.QueryVideoMemoryInfo(0, grp, &mut info).is_ok() {
                        info.CurrentUsage as f32 / (1024.0 * 1024.0)
                    } else {
                        0.0
                    }
                };
                used_mb = usage(DXGI_MEMORY_SEGMENT_GROUP_LOCAL);
                if small_carveout {
                    used_mb += usage(DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL);
                }
            }
            // Pick the adapter with the most DEDICATED VRAM (so a discrete GPU still
            // wins over an iGPU whose shared budget can be larger).
            if dedicated_mb > best_dedicated {
                best_dedicated = dedicated_mb;
                best = Some((name, used_mb, total_mb, luid));
            }
        }
        best
    }
}

#[cfg(not(windows))]
fn dxgi_query() -> Option<(String, f32, f32, u64)> {
    None
}

/// Human-readable GPU diagnostic for the hidden `--gpu-debug` flag. Cross-platform:
/// shows the selected backend and its live metrics on any OS, and on Windows
/// additionally dumps the raw DXGI adapters + per-node D3DKMT data used to derive
/// the clock and VRAM — so a clock/VRAM bug on a given GPU (Intel, an old card,
/// etc.) can be diagnosed from real numbers rather than guessed at.
pub fn gpu_debug_report() -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let mut src = select_gpu_source();
    let id = src.identity();
    let _ = writeln!(s, "=== Flux GPU debug ===");
    let _ = writeln!(s, "OS: {}   arch: {}", std::env::consts::OS, std::env::consts::ARCH);
    let _ = writeln!(s, "backend: {}", src.kind());
    let _ = writeln!(s, "name: {:?}   integrated: {}", id.name, id.is_integrated);
    let _ = writeln!(s, "-- 6 live samples (~0.4s apart; run a game/benchmark for a non-idle clock) --");
    for i in 0..6 {
        let m = src.read();
        let f = |o: Option<f32>, suf: &str| o.map(|v| format!("{v:.0}{suf}")).unwrap_or_else(|| "-".into());
        let _ = writeln!(
            s,
            "  [{i}] usage={:>3.0}%  temp={:<5}  clock={:<8}  vram={:.0}/{:.0} MB  fan={}",
            m.usage_percent,
            f(m.temperature_c, "C"),
            f(m.clock_mhz, "MHz"),
            m.vram_used_mb,
            m.vram_total_mb,
            f(m.fan_rpm, "")
        );
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    #[cfg(windows)]
    {
        let _ = writeln!(s, "\n-- raw DXGI adapters + D3DKMT per-node --");
        s.push_str(&dxgi_raw_dump());
    }
    s
}

/// Windows-only: enumerate every DXGI adapter with its raw memory figures and,
/// for each non-software adapter, the per-node D3DKMT clock dump.
#[cfg(windows)]
fn dxgi_raw_dump() -> String {
    use std::fmt::Write;
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
        DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
        DXGI_QUERY_VIDEO_MEMORY_INFO,
    };
    let mut s = String::new();
    unsafe {
        let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
            Ok(f) => f,
            Err(e) => return format!("  CreateDXGIFactory1 failed: {e:?}\n"),
        };
        let mut i = 0u32;
        loop {
            let adapter = match factory.EnumAdapters1(i) {
                Ok(a) => a,
                Err(_) => break,
            };
            i += 1;
            let desc = match adapter.GetDesc1() {
                Ok(d) => d,
                Err(_) => continue,
            };
            let raw = String::from_utf16_lossy(&desc.Description);
            let name = raw.trim_end_matches('\0').trim();
            let software = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
            let _ = writeln!(s, "adapter {i}: {name:?}{}", if software { "  [software]" } else { "" });
            let _ = writeln!(
                s,
                "  dedicated={:.0} MB  shared={:.0} MB",
                desc.DedicatedVideoMemory as f64 / 1_048_576.0,
                desc.SharedSystemMemory as f64 / 1_048_576.0
            );
            if let Ok(a3) = adapter.cast::<IDXGIAdapter3>() {
                for (label, grp) in [
                    ("LOCAL    ", DXGI_MEMORY_SEGMENT_GROUP_LOCAL),
                    ("NON_LOCAL", DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL),
                ] {
                    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                    if a3.QueryVideoMemoryInfo(0, grp, &mut info).is_ok() {
                        let _ = writeln!(
                            s,
                            "  {label}: used={:.0} MB  budget={:.0} MB",
                            info.CurrentUsage as f64 / 1_048_576.0,
                            info.Budget as f64 / 1_048_576.0
                        );
                    }
                }
            } else {
                let _ = writeln!(s, "  (no IDXGIAdapter3 — pre-Win10 memory-info API)");
            }
            if software {
                continue;
            }
            s.push_str(&d3dkmt::debug_dump(desc.AdapterLuid));
        }
    }
    s
}

// Heuristic: is this GPU an integrated/on-die part that shares the CPU's thermal
// sensor (so it has no separate temperature)?
//
// Every vendor ships BOTH integrated and discrete parts, so correctness here is
// "never call a discrete card integrated" (that would hide its real temp) over
// "catch every iGPU". We therefore VETO on discrete markers first, then match the
// known integrated families. NVIDIA has no PC iGPU, so any NVIDIA name is discrete
// (and NVIDIA normally goes through NVML, never reaching this).
fn gpu_name_is_integrated(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // 1) Discrete markers veto integration outright.
    const DISCRETE: [&str; 12] = [
        "geforce", "nvidia", "rtx", "gtx", "quadro", "tesla", "titan",
        "radeon rx", "radeon pro", "firepro", "instinct", "arc",
    ];
    if DISCRETE.iter().any(|d| n.contains(d)) {
        return false;
    }
    // 2) Intel integrated: UHD / HD / Iris, or a generic "Intel … Graphics".
    if n.contains("uhd graphics") || n.contains("hd graphics") || n.contains("iris") {
        return true;
    }
    if n.contains("intel") && n.contains("graphics") {
        return true;
    }
    // 3) AMD APU iGPUs: "Radeon(TM) Graphics", "Radeon Vega N Graphics", and the
    //    mobile parts "Radeon 6xxM/7xxM/8xxM" (discrete "Radeon RX/Pro" vetoed above).
    if n.contains("radeon") && n.contains("graphics") {
        return true;
    }
    if n.contains("radeon")
        && ["610m", "660m", "680m", "740m", "760m", "780m", "860m", "880m", "890m"]
            .iter()
            .any(|m| n.contains(m))
    {
        return true;
    }
    // 4) ARM / Qualcomm / Apple integrated.
    n.contains("adreno") || n.contains("mali") || n.contains("apple m")
}

#[cfg(test)]
mod integrated_gpu_tests {
    use super::gpu_name_is_integrated;

    #[test]
    fn discrete_cards_are_never_integrated() {
        for name in [
            "NVIDIA GeForce RTX 4090", "NVIDIA RTX A4000", "NVIDIA GeForce GTX 1660",
            "AMD Radeon RX 7900 XTX", "AMD Radeon RX 7600M XT", "AMD Radeon Pro W7900",
            "AMD Radeon RX Vega 64", "Intel(R) Arc(TM) A770 Graphics", "Intel Arc Graphics",
        ] {
            assert!(!gpu_name_is_integrated(name), "{name} wrongly integrated");
        }
    }

    #[test]
    fn integrated_parts_are_detected() {
        for name in [
            "AMD Radeon(TM) Graphics", "AMD Radeon(TM) 780M Graphics", "AMD Radeon 680M",
            "AMD Radeon(TM) Vega 8 Graphics", "Intel(R) UHD Graphics 770",
            "Intel(R) Iris(R) Xe Graphics", "Intel(R) HD Graphics 630",
        ] {
            assert!(gpu_name_is_integrated(name), "{name} not detected as integrated");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  GPU: Apple Silicon (macOS). IOKit/Metal not yet wired; degrades to None
//  so the GPU tile renders em-dashes rather than failing.
// ─────────────────────────────────────────────────────────────────────────
#[cfg(target_os = "macos")]
fn apple_gpu_query() -> Option<(String, f32, f32, Option<f32>)> {
    // TODO: IOKit IOAccelerator / Metal MTLDevice for name, usage, VRAM, temp.
    None
}

// ─────────────────────────────────────────────────────────────────────────
//  CPU temperature backends
// ─────────────────────────────────────────────────────────────────────────
// Windows: WMI MSAcpi_ThermalZoneTemperature (root\WMI). No elevation needed.
// CurrentTemperature is reported in tenths of a Kelvin. Many systems expose a
// motherboard thermal zone rather than the CPU die, so this is a coarse
// fallback used only when a hardware-monitor driver isn't feeding sysinfo.
#[cfg(windows)]
fn wmi_cpu_temp() -> Option<f32> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use wmi::{COMLibrary, Variant, WMIConnection};
    thread_local! {
        static CONN: RefCell<Option<WMIConnection>> = const { RefCell::new(None) };
    }
    CONN.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            // winit initializes COM (STA) on the main thread already, so assume
            // it rather than re-initializing with a conflicting mode.
            let com = unsafe { COMLibrary::assume_initialized() };
            *guard = WMIConnection::with_namespace_path("root\\WMI", com).ok();
        }
        let conn = guard.as_ref()?;
        let results: Vec<HashMap<String, Variant>> = conn
            .raw_query("SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature")
            .ok()?;
        let mut best: Option<f32> = None;
        for row in results {
            if let Some(v) = row.get("CurrentTemperature") {
                let raw: f64 = match v {
                    Variant::UI4(n) => *n as f64,
                    Variant::I4(n) => *n as f64,
                    Variant::UI2(n) => *n as f64,
                    Variant::I2(n) => *n as f64,
                    _ => continue,
                };
                let c = (raw / 10.0) - 273.15;
                // A CPU die is essentially never below ~20 °C. Many boards expose
                // only a cool ambient/chipset zone here; showing that as the CPU
                // temp is misleading, so reject implausibly-low values and let the
                // tile show "—" instead of a wrong number.
                if (20.0..130.0).contains(&c) {
                    let cf = c as f32;
                    best = Some(best.map_or(cf, |b| b.max(cf)));
                }
            }
        }
        best
    })
}

// Accurate CPU package temperature from LibreHardwareMonitor / OpenHardwareMonitor
// if either is running (they expose a WMI `Sensor` class). This needs no driver
// of our own — we just read their data. Prefers "CPU Package", else the hottest
// CPU core. The connection result is cached (incl. "not available") so we don't
// probe a missing namespace every poll.
#[cfg(windows)]
fn lhm_cpu_temp() -> Option<f32> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use wmi::{COMLibrary, Variant, WMIConnection};
    thread_local! {
        // Outer None = not yet probed; inner None = no hardware-monitor present.
        static CONN: RefCell<Option<Option<WMIConnection>>> = const { RefCell::new(None) };
    }
    CONN.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            let mut conn = None;
            for ns in ["root\\LibreHardwareMonitor", "root\\OpenHardwareMonitor"] {
                let com = unsafe { COMLibrary::assume_initialized() };
                if let Ok(c) = WMIConnection::with_namespace_path(ns, com) {
                    // Confirm the Sensor class actually exists in this namespace.
                    if c.raw_query::<HashMap<String, Variant>>("SELECT Name FROM Sensor").is_ok() {
                        conn = Some(c);
                        break;
                    }
                }
            }
            *guard = Some(conn);
        }
        let conn = guard.as_ref().unwrap().as_ref()?;
        let rows: Vec<HashMap<String, Variant>> = conn
            .raw_query("SELECT Name, Value FROM Sensor WHERE SensorType = 'Temperature'")
            .ok()?;
        let mut package: Option<f32> = None;
        let mut core_max: Option<f32> = None;
        for row in rows {
            let name = match row.get("Name") {
                Some(Variant::String(s)) => s.to_lowercase(),
                _ => continue,
            };
            let val: f32 = match row.get("Value") {
                Some(Variant::R4(f)) => *f,
                Some(Variant::R8(f)) => *f as f32,
                _ => continue,
            };
            if !(0.0..=150.0).contains(&val) || !name.contains("cpu") {
                continue;
            }
            if name.contains("package") {
                package = Some(val);
            } else if name.contains("core") || name.contains("tctl") {
                core_max = Some(core_max.map_or(val, |m: f32| m.max(val)));
            }
        }
        package.or(core_max)
    })
}

// RAM type + rated speed via WMI Win32_PhysicalMemory (root\CIMV2). Static for
// the machine, so the result is cached after the first successful read.
#[cfg(windows)]
fn wmi_ram_info() -> Option<(u32, String)> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use wmi::{COMLibrary, Variant, WMIConnection};
    thread_local! {
        static CONN: RefCell<Option<WMIConnection>> = const { RefCell::new(None) };
    }
    CONN.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            let com = unsafe { COMLibrary::assume_initialized() };
            *guard = WMIConnection::new(com).ok();
        }
        let conn = guard.as_ref()?;
        let rows: Vec<HashMap<String, Variant>> = conn
            .raw_query("SELECT Speed, SMBIOSMemoryType FROM Win32_PhysicalMemory")
            .ok()?;
        let row = rows.into_iter().next()?;
        let as_u32 = |v: Option<&Variant>| -> u32 {
            match v {
                Some(Variant::UI4(n)) => *n,
                Some(Variant::UI2(n)) => *n as u32,
                Some(Variant::I4(n)) => *n as u32,
                Some(Variant::I2(n)) => *n as u32,
                _ => 0,
            }
        };
        let speed = as_u32(row.get("Speed"));
        let mem_type = match as_u32(row.get("SMBIOSMemoryType")) {
            20 => "DDR", 21 => "DDR2", 24 => "DDR3", 26 => "DDR4", 34 => "DDR5",
            _ => "",
        }.to_string();
        if speed == 0 { return None; }
        Some((speed, mem_type))
    })
}

// Cached RAM type/speed (queried once; the value never changes at runtime).
fn ram_info_cached() -> (u32, String) {
    use std::sync::OnceLock;
    static C: OnceLock<(u32, String)> = OnceLock::new();
    C.get_or_init(|| {
        #[cfg(windows)]
        { wmi_ram_info().unwrap_or((0, String::new())) }
        #[cfg(not(windows))]
        { (0, String::new()) }
    }).clone()
}

// Physical-disk model for a drive letter (e.g. "C:") via WMI association walk
// LogicalDisk -> Partition -> DiskDrive.Model, mirroring the C# service. sysinfo
// only exposes the volume label (usually empty on Windows), so the tile's
// "Model" label needs this. Cached per thread (models don't change at runtime).
#[cfg(windows)]
fn disk_model_for(mount: &str) -> Option<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static MAP: RefCell<Option<HashMap<String, String>>> = const { RefCell::new(None) };
    }
    let letter = mount.trim_end_matches('\\').to_uppercase();
    if letter.is_empty() {
        return None;
    }
    MAP.with(|cell| {
        let mut g = cell.borrow_mut();
        if g.is_none() {
            *g = Some(build_disk_model_map());
        }
        g.as_ref().unwrap().get(&letter).cloned()
    })
}

#[cfg(windows)]
fn build_disk_model_map() -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    use wmi::{COMLibrary, Variant, WMIConnection};
    let mut map: HashMap<String, String> = HashMap::new();
    let com = unsafe { COMLibrary::assume_initialized() };
    let conn = match WMIConnection::with_namespace_path("root\\CIMV2", com) {
        Ok(c) => c,
        Err(_) => return map,
    };
    let str_of = |row: &HashMap<String, Variant>, k: &str| -> Option<String> {
        match row.get(k) {
            Some(Variant::String(s)) => Some(s.clone()),
            _ => None,
        }
    };
    let logicals: Vec<HashMap<String, Variant>> = conn
        .raw_query("SELECT DeviceID FROM Win32_LogicalDisk WHERE DriveType = 3")
        .unwrap_or_default();
    for ld in &logicals {
        let Some(letter) = str_of(ld, "DeviceID") else { continue };
        // LogicalDisk -> Partition(s)
        let q_parts = format!(
            "ASSOCIATORS OF {{Win32_LogicalDisk.DeviceID='{letter}'}} WHERE AssocClass=Win32_LogicalDiskToPartition"
        );
        let parts: Vec<HashMap<String, Variant>> = conn.raw_query(&q_parts).unwrap_or_default();
        'outer: for part in &parts {
            let Some(pid) = str_of(part, "DeviceID") else { continue };
            // Partition -> DiskDrive(s) (has Model)
            let q_drives = format!(
                "ASSOCIATORS OF {{Win32_DiskPartition.DeviceID='{pid}'}} WHERE AssocClass=Win32_DiskDriveToDiskPartition"
            );
            let drives: Vec<HashMap<String, Variant>> = conn.raw_query(&q_drives).unwrap_or_default();
            for drive in &drives {
                if let Some(model) = str_of(drive, "Model") {
                    let model = model.trim().to_string();
                    if !model.is_empty() {
                        map.insert(letter.to_uppercase(), model);
                        break 'outer;
                    }
                }
            }
        }
    }
    map
}

#[cfg(target_os = "linux")]
fn linux_cpu_temp() -> Option<f32> {
    use std::fs;
    // Prefer hwmon coretemp / k10temp "Package"/"Tctl" labels.
    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let base = entry.path();
            for i in 1..=8 {
                let label_path = base.join(format!("temp{}_label", i));
                let input_path = base.join(format!("temp{}_input", i));
                if let Ok(label) = fs::read_to_string(&label_path) {
                    let l = label.to_lowercase();
                    if l.contains("package") || l.contains("tctl") || l.contains("tdie") {
                        if let Ok(v) = fs::read_to_string(&input_path) {
                            if let Ok(milli) = v.trim().parse::<f32>() {
                                return Some(milli / 1000.0);
                            }
                        }
                    }
                }
            }
        }
    }
    // Fallback: thermal_zone0
    if let Ok(v) = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
        if let Ok(milli) = v.trim().parse::<f32>() {
            let c = milli / 1000.0;
            if c > 0.0 {
                return Some(c);
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_cpu_temp() -> Option<f32> {
    // TODO: IOKit SMC read of TC0P/TC1P keys. Degrades to None for now.
    None
}

impl Default for SensorPoller {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorPoller {
    pub fn new() -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        // Pick the GPU backend once and cache its static identity; log which one.
        let gpu_source = select_gpu_source();
        let gpu_identity = gpu_source.identity();
        tracing::info!(
            "GPU source: {} — '{}'{}",
            gpu_source.kind(),
            gpu_identity.name,
            if gpu_identity.is_integrated { " (integrated)" } else { "" }
        );

        Self {
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            gpu_identity,
            gpu_source,
            cpu_freq: CpuFreq::new(),
        }
    }

    pub fn poll(&mut self) -> SensorSnapshot {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.disks.refresh(true);
        self.networks.refresh(true);
        self.components.refresh(true);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        SensorSnapshot {
            cpu: self.read_cpu(),
            gpu: self.read_gpu(),
            ram: self.read_ram(),
            disk: self.read_disks(),
            network: self.read_network(),
            timestamp,
        }
    }

    fn read_cpu(&mut self) -> CpuData {
        let cpus = self.system.cpus();
        let global_usage = self.system.global_cpu_usage();
        let name = cpus.first()
            .map(|c| shorten_cpu_name(c.brand()))
            .unwrap_or_default();
        // sysinfo gives the static base MHz; scale it by the PDH performance
        // ratio for the live boost-aware clock (Task Manager "Speed"). Falls
        // back to the base value before the counter is primed or off Windows.
        let base = cpus.first().map(|c| c.frequency() as f32);
        let clock = match (self.cpu_freq.as_mut().and_then(|f| f.current_ratio()), base) {
            (Some(ratio), Some(b)) => Some(b * ratio),
            _ => base,
        };

        let temp = self.cpu_temperature();

        CpuData {
            name,
            usage_percent: global_usage,
            temperature_c: temp,
            clock_mhz: clock,
            core_count: System::physical_core_count().unwrap_or(0) as u32,
            thread_count: cpus.len() as u32,
            per_core_usage: cpus.iter().map(|c| c.cpu_usage()).collect(),
        }
    }

    fn cpu_temperature(&self) -> Option<f32> {
        // sysinfo components (works on Linux always, on Windows only with a
        // hardware-monitor driver providing the label).
        let from_components = self.components.iter()
            .find(|c| {
                let label = c.label().to_lowercase();
                label.contains("cpu") || label.contains("package") || label.contains("tctl")
            })
            .and_then(|c| c.temperature())
            .filter(|t| *t > 0.0);
        if from_components.is_some() {
            return from_components;
        }

        #[cfg(windows)]
        {
            // Preferred: the elevated Flux sensor service publishes the PawnIO
            // reading here, so the (non-elevated) widget needn't run as admin.
            if let Some(r) = flux_core::sensor_ipc::read() {
                if flux_core::sensor_ipc::now_unix().saturating_sub(r.updated_unix)
                    <= flux_core::sensor_ipc::FRESH_SECS
                {
                    if let Some(t) = r.cpu_temp {
                        return Some(t);
                    }
                }
            }
            // Direct read — works when the widget itself is elevated (run as
            // admin) or when no service is installed.
            if let Some(t) = pawnio::cpu_temp() {
                return Some(t);
            }
            // Otherwise prefer an accurate reading from a running hardware
            // monitor; fall back to the coarse ACPI thermal zone.
            if let Some(t) = lhm_cpu_temp() {
                return Some(t);
            }
            if let Some(t) = wmi_cpu_temp() {
                return Some(t);
            }
        }
        #[cfg(target_os = "linux")]
        {
            if let Some(t) = linux_cpu_temp() {
                return Some(t);
            }
        }
        #[cfg(target_os = "macos")]
        {
            if let Some(t) = macos_cpu_temp() {
                return Some(t);
            }
        }
        None
    }

    fn read_gpu(&mut self) -> GpuData {
        // Live numbers from the active backend, merged with the cached identity.
        // The components-temperature fallback lives here (not in the backend) so
        // every source benefits from it when it can't read a GPU temp itself.
        let m = self.gpu_source.read();
        let temperature_c = m.temperature_c.or_else(|| self.gpu_temp_from_components());
        GpuData {
            name: self.gpu_identity.name.clone(),
            is_integrated: self.gpu_identity.is_integrated,
            usage_percent: m.usage_percent,
            temperature_c,
            clock_mhz: m.clock_mhz,
            vram_used_mb: m.vram_used_mb,
            vram_total_mb: m.vram_total_mb,
            fan_rpm: m.fan_rpm,
        }
    }

    fn gpu_temp_from_components(&self) -> Option<f32> {
        self.components.iter()
            .find(|c| c.label().to_lowercase().contains("gpu"))
            .and_then(|c| c.temperature())
            .filter(|t| *t > 0.0)
    }

    fn read_ram(&self) -> RamData {
        let used = self.system.used_memory() as f32 / (1024.0 * 1024.0);
        let total = self.system.total_memory() as f32 / (1024.0 * 1024.0);
        let (speed_mhz, mem_type) = ram_info_cached();
        RamData {
            used_mb: used,
            total_mb: total,
            usage_percent: if total > 0.0 { (used / total) * 100.0 } else { 0.0 },
            speed_mhz,
            mem_type,
        }
    }

    fn read_disks(&self) -> DiskData {
        let mut drives: Vec<DriveInfo> = self.disks.iter().map(|d| {
            let total = d.total_space() as f32 / (1024.0 * 1024.0 * 1024.0);
            let available = d.available_space() as f32 / (1024.0 * 1024.0 * 1024.0);
            let usage = d.usage();
            let mount = d.mount_point().to_string_lossy().to_string();
            // Prefer the physical-disk model (WMI on Windows); fall back to the
            // volume label sysinfo provides.
            let name = {
                #[cfg(windows)]
                { disk_model_for(&mount).unwrap_or_else(|| d.name().to_string_lossy().to_string()) }
                #[cfg(not(windows))]
                { d.name().to_string_lossy().to_string() }
            };
            DriveInfo {
                name,
                mount,
                total_gb: total,
                used_gb: total - available,
                read_bytes_sec: usage.read_bytes,
                write_bytes_sec: usage.written_bytes,
            }
        }).collect();

        // C: drive first, rest in mount order
        drives.sort_by_key(|d| if d.mount.starts_with("C:") { 0 } else { 1 });

        DiskData { drives }
    }

    fn read_network(&mut self) -> NetworkData {
        let interfaces = self.networks.iter().map(|(name, data)| {
            NetInterface {
                name: name.clone(),
                upload_bytes_sec: data.transmitted(),
                download_bytes_sec: data.received(),
                total_uploaded: data.total_transmitted(),
                total_downloaded: data.total_received(),
            }
        }).collect();

        NetworkData { interfaces }
    }
}
