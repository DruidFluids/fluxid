use fluid_core::sensor_data::*;
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use sysinfo::{Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq)]
enum GpuBackend {
    Nvml,
    Dxgi,
    Components,
}

pub struct SensorPoller {
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    nvml: Option<Nvml>,
    gpu_backend: GpuBackend,
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
#[cfg(windows)]
fn dxgi_query() -> Option<(String, f32, f32)> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_ADAPTER_DESC1,
        DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
    };
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut best: Option<(String, f32, f32)> = None;
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
            let total_mb = desc.DedicatedVideoMemory as f32 / (1024.0 * 1024.0);
            let mut used_mb = 0.0f32;
            if let Ok(adapter3) = adapter.cast::<IDXGIAdapter3>() {
                let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                if adapter3
                    .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                    .is_ok()
                {
                    used_mb = info.CurrentUsage as f32 / (1024.0 * 1024.0);
                }
            }
            // Prefer the adapter with the most dedicated VRAM (the discrete GPU)
            if best.as_ref().map_or(true, |b| total_mb > b.2) {
                best = Some((name, used_mb, total_mb));
            }
        }
        best
    }
}

#[cfg(not(windows))]
fn dxgi_query() -> Option<(String, f32, f32)> {
    None
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
                if c > 0.0 && c < 150.0 {
                    let cf = c as f32;
                    best = Some(best.map_or(cf, |b| b.max(cf)));
                }
            }
        }
        best
    })
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

impl SensorPoller {
    pub fn new() -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        let nvml = Nvml::init().ok();

        // Decide and log the GPU backend that will drive the GPU tile.
        let gpu_backend = if let Some(n) = &nvml {
            if n.device_by_index(0).is_ok() {
                GpuBackend::Nvml
            } else {
                Self::detect_non_nvml_backend()
            }
        } else {
            Self::detect_non_nvml_backend()
        };

        match gpu_backend {
            GpuBackend::Nvml => {
                tracing::info!("GPU backend: NVML (NVIDIA) — name, load, temp, VRAM, clock");
            }
            GpuBackend::Dxgi => {
                if let Some((name, _, total)) = dxgi_query() {
                    tracing::info!(
                        "GPU backend: DXGI — '{}' ({:.0} MB VRAM); temp/clock unavailable",
                        name,
                        total
                    );
                } else {
                    tracing::info!("GPU backend: DXGI");
                }
            }
            GpuBackend::Components => {
                tracing::info!("GPU backend: sysinfo components (temp only)");
            }
        }

        Self {
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            nvml,
            gpu_backend,
        }
    }

    fn detect_non_nvml_backend() -> GpuBackend {
        #[cfg(target_os = "macos")]
        {
            if apple_gpu_query().is_some() {
                return GpuBackend::Dxgi; // reuse "vendor SDK" path label
            }
        }
        if dxgi_query().is_some() {
            return GpuBackend::Dxgi;
        }
        GpuBackend::Components
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

    fn read_cpu(&self) -> CpuData {
        let cpus = self.system.cpus();
        let global_usage = self.system.global_cpu_usage();
        let name = cpus.first()
            .map(|c| shorten_cpu_name(c.brand()))
            .unwrap_or_default();
        let clock = cpus.first().map(|c| c.frequency() as f32);

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

    fn read_gpu(&self) -> GpuData {
        // NVIDIA via NVML — full data.
        if self.gpu_backend == GpuBackend::Nvml {
            if let Some(nvml) = &self.nvml {
                if let Ok(device) = nvml.device_by_index(0) {
                    let name = device.name().unwrap_or_else(|_| "GPU".into())
                        .replace("NVIDIA ", "");
                    let usage = device.utilization_rates()
                        .map(|u| u.gpu as f32)
                        .unwrap_or(0.0);
                    let temp = device.temperature(TemperatureSensor::Gpu)
                        .ok()
                        .map(|t| t as f32);
                    let (vram_used, vram_total) = device.memory_info()
                        .map(|m| (
                            m.used as f32 / (1024.0 * 1024.0),
                            m.total as f32 / (1024.0 * 1024.0),
                        ))
                        .unwrap_or((0.0, 0.0));
                    let clock = device.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
                        .ok()
                        .map(|c| c as f32);

                    return GpuData {
                        name,
                        usage_percent: usage,
                        temperature_c: temp,
                        vram_used_mb: vram_used,
                        vram_total_mb: vram_total,
                        clock_mhz: clock,
                        ..Default::default()
                    };
                }
            }
        }

        // Apple Silicon (macOS).
        #[cfg(target_os = "macos")]
        {
            if let Some((name, used, total, temp)) = apple_gpu_query() {
                return GpuData {
                    name,
                    vram_used_mb: used,
                    vram_total_mb: total,
                    temperature_c: temp,
                    ..Default::default()
                };
            }
        }

        // DXGI (AMD / Intel / unrecognized) — name + VRAM only.
        if self.gpu_backend == GpuBackend::Dxgi {
            if let Some((name, used, total)) = dxgi_query() {
                let temp = self.gpu_temp_from_components();
                return GpuData {
                    name,
                    temperature_c: temp,
                    vram_used_mb: used,
                    vram_total_mb: total,
                    ..Default::default()
                };
            }
        }

        // Last resort: temperature from sysinfo components only.
        GpuData {
            name: "GPU".into(),
            temperature_c: self.gpu_temp_from_components(),
            ..Default::default()
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
        RamData {
            used_mb: used,
            total_mb: total,
            usage_percent: if total > 0.0 { (used / total) * 100.0 } else { 0.0 },
        }
    }

    fn read_disks(&self) -> DiskData {
        let mut drives: Vec<DriveInfo> = self.disks.iter().map(|d| {
            let total = d.total_space() as f32 / (1024.0 * 1024.0 * 1024.0);
            let available = d.available_space() as f32 / (1024.0 * 1024.0 * 1024.0);
            let usage = d.usage();
            DriveInfo {
                name: d.name().to_string_lossy().to_string(),
                mount: d.mount_point().to_string_lossy().to_string(),
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
