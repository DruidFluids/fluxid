//! Serializable sensor snapshot types shared across the service, IPC, and widget.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SensorSnapshot {
    pub cpu: CpuData,
    pub gpu: GpuData,
    pub ram: RamData,
    pub disk: DiskData,
    pub network: NetworkData,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuData {
    pub name: String,
    pub usage_percent: f32,
    pub temperature_c: Option<f32>,
    pub clock_mhz: Option<f32>,
    pub core_count: u32,
    pub thread_count: u32,
    pub per_core_usage: Vec<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuData {
    pub name: String,
    pub usage_percent: f32,
    pub temperature_c: Option<f32>,
    pub clock_mhz: Option<f32>,
    pub vram_used_mb: f32,
    pub vram_total_mb: f32,
    pub fan_rpm: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RamData {
    pub used_mb: f32,
    pub total_mb: f32,
    pub usage_percent: f32,
    pub speed_mhz: u32,
    pub mem_type: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskData {
    pub drives: Vec<DriveInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriveInfo {
    pub name: String,
    pub mount: String,
    pub used_gb: f32,
    pub total_gb: f32,
    pub read_bytes_sec: u64,
    pub write_bytes_sec: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkData {
    pub interfaces: Vec<NetInterface>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetInterface {
    pub name: String,
    pub upload_bytes_sec: u64,
    pub download_bytes_sec: u64,
    pub total_uploaded: u64,
    pub total_downloaded: u64,
}

