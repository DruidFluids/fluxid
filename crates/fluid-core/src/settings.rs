use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    // Appearance
    pub theme_bg: String,
    pub theme_tile: String,
    pub theme_accent: String,
    pub theme_text: String,
    pub theme_muted: String,
    pub active_skin: String,
    pub primary_font: Option<String>,
    pub secondary_font: Option<String>,
    pub indicator_font: Option<String>,
    pub font_size_offset: f32,

    // Layout
    pub orientation: Orientation,
    pub tile_order: Vec<String>,
    pub visible_tiles: Vec<String>,
    pub widget_opacity: f32,
    pub click_through: bool,

    // Position
    pub window_x: f64,
    pub window_y: f64,
    pub settings_window_x: Option<f64>,
    pub settings_window_y: Option<f64>,
    pub snap_to_edges: bool,

    // Game mode
    pub game_mode_enabled: bool,
    pub game_mode_hotkey: String,
    pub game_mode_position: SnapPosition,
    pub game_mode_opacity: f32,
    pub game_mode_tiles: Vec<String>,

    // Alerts
    pub alert_cpu_threshold: f32,
    pub alert_gpu_threshold: f32,
    pub alert_ram_threshold: f32,
    pub alert_mode: AlertMode,

    // Remote monitoring
    pub remote_enabled: bool,
    pub remote_port: u16,
    pub remote_key: String,
    pub remote_devices: Vec<RemoteDevice>,

    // Updates
    pub update_check_mode: UpdateMode,
    pub last_update_check: Option<String>,

    // Presets (quick slots)
    pub presets: Vec<PresetSlot>,

    // Misc
    pub temperature_unit: TempUnit,
    pub start_minimized: bool,
    pub first_run_complete: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_bg: "#FF1A1A1E".into(),
            theme_tile: "#FF242428".into(),
            theme_accent: "#FF3A8FD4".into(),
            theme_text: "#FFE8E8E8".into(),
            theme_muted: "#FF888888".into(),
            active_skin: "Default".into(),
            primary_font: None,
            secondary_font: None,
            indicator_font: None,
            font_size_offset: 0.0,

            orientation: Orientation::Vertical,
            tile_order: vec![
                "CPU".into(), "GPU".into(), "RAM".into(),
                "Disk".into(), "Network".into(),
            ],
            visible_tiles: vec![
                "CPU".into(), "GPU".into(), "RAM".into(),
                "Disk".into(), "Network".into(),
            ],
            widget_opacity: 1.0,
            click_through: false,

            window_x: 100.0,
            window_y: 100.0,
            settings_window_x: None,
            settings_window_y: None,
            snap_to_edges: true,

            game_mode_enabled: false,
            game_mode_hotkey: "Ctrl+G".into(),
            game_mode_position: SnapPosition::TopRight,
            game_mode_opacity: 0.8,
            game_mode_tiles: vec!["CPU".into(), "GPU".into(), "RAM".into()],

            alert_cpu_threshold: 85.0,
            alert_gpu_threshold: 85.0,
            alert_ram_threshold: 90.0,
            alert_mode: AlertMode::Flash,

            remote_enabled: false,
            remote_port: 5199,
            remote_key: String::new(),
            remote_devices: Vec::new(),

            update_check_mode: UpdateMode::Manual,
            last_update_check: None,

            presets: Vec::new(),

            temperature_unit: TempUnit::Celsius,
            start_minimized: false,
            first_run_complete: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SnapPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertMode {
    Off,
    Flash,
    Gradient,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateMode {
    Auto,
    Manual,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TempUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSlot {
    pub name: String,
    pub bg: String,
    pub tile: String,
    pub accent: String,
    pub text: String,
    pub muted: String,
    pub skin: String,
}

impl AppSettings {
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "fluidmonitor", "fluidMonitor")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let json = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&json)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }
}
