//! Persisted application settings (`settings.json`) and their defaults.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme_bg: String,
    pub theme_tile: String,
    pub theme_accent: String,
    pub theme_text: String,
    pub theme_muted: String,
    pub active_skin: String,
    pub muted_contrast: f32,

    pub primary_font: Option<String>,
    pub secondary_font: Option<String>,
    pub indicator_font: Option<String>,
    pub primary_font_offset: i32,
    pub secondary_font_offset: i32,
    pub indicator_font_offset: i32,

    pub tile_width: f32,
    pub tile_height: f32,
    pub cpu_custom_name: String,
    pub gpu_custom_name: String,
    pub selected_disk_mount: String,
    pub network_adapter_name: String,

    pub orientation: Orientation,
    pub tile_order: Vec<String>,
    pub visible_tiles: Vec<String>,
    pub widget_opacity: f32,
    pub click_through: bool,
    pub always_on_top: bool,
    pub round_corners: bool,

    pub update_interval_ms: u64,
    pub run_at_startup: bool,
    pub ui_scale: f32,
    pub snap_to_windows: bool,
    pub click_through_hotkey: String,

    pub network_traffic_indicator: String,
    pub network_arrow_spacing: f32,
    pub arrow_font_offset: i32,

    pub disk_label_spacing: f32,
    pub disk_label_font_offset: i32,
    pub disk_label_style: String,

    pub sync_fonts: bool,
    pub randomize_fonts_on_dice: bool,

    pub window_x: f64,
    pub window_y: f64,
    pub settings_window_x: Option<f64>,
    pub settings_window_y: Option<f64>,
    pub snap_to_edges: bool,
    pub snap_distance: f32,

    pub game_mode_enabled: bool,
    pub game_mode_hotkey: String,
    pub game_mode_position: SnapPosition,
    pub game_mode_opacity: f32,
    pub game_mode_orientation: String,
    pub game_mode_click_through: bool,
    pub game_mode_tiles: Vec<String>,

    pub warnings: Vec<TileWarning>,

    pub remote_enabled: bool,
    pub remote_port: u16,
    pub remote_key: String,
    pub remote_devices: Vec<RemoteDevice>,

    #[serde(default)]
    pub snap_blocklist: Vec<String>,
    /// Set once the Windows Firewall rule for the TCP feed has been added, so we
    /// don't re-prompt for elevation on every enable.
    #[serde(default)]
    pub remote_firewall_configured: bool,
    /// Last on-screen position of each popup/sub-window, keyed by window kind,
    /// so they reopen where the user left them.
    #[serde(default)]
    pub popup_positions: std::collections::HashMap<String, (f64, f64)>,

    pub update_check_mode: UpdateMode,
    pub last_update_check: Option<String>,
    pub presets: Vec<PresetSlot>,
    /// Game-pack themes the user installed from the Theme Store. These show up
    /// in the "Choose a Theme" list alongside the built-in presets.
    pub installed_themes: Vec<PresetSlot>,
    pub temperature_unit: TempUnit,
    pub start_minimized: bool,
    pub first_run_complete: bool,
    /// User dismissed the CPU-tile "turn on temperature" hint (the in-tile
    /// nudge to install the optional sensor driver). Resets if they later open
    /// the driver dialog from Settings.
    pub cpu_temp_hint_dismissed: bool,
    /// Show a green/red connection-status dot on the widget's remote-device
    /// switcher tabs.
    pub show_remote_status_dot: bool,

    // ── Per-tile field visibility (all default ON) ──
    #[serde(default = "def_true")] pub cpu_show_temp: bool,
    #[serde(default = "def_true")] pub cpu_show_clock: bool,
    #[serde(default = "def_true")] pub gpu_show_temp: bool,
    #[serde(default = "def_true")] pub gpu_show_clock: bool,
    #[serde(default = "def_true")] pub gpu_show_vram: bool,
    #[serde(default = "def_true")] pub ram_show_speed: bool,
    #[serde(default = "def_true")] pub ram_show_details: bool,
    #[serde(default = "def_true")] pub net_show_down: bool,
    #[serde(default = "def_true")] pub net_show_up: bool,
    #[serde(default = "def_true")] pub disk_show_read: bool,
    #[serde(default = "def_true")] pub disk_show_write: bool,
    #[serde(default = "def_true")] pub clock_show_date: bool,
}

fn def_true() -> bool { true }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_bg: "#E61E1E22".into(),
            theme_tile: "#FF2A2A30".into(),
            theme_accent: "#FF00A8FF".into(),
            theme_text: "#FFE8E8EC".into(),
            theme_muted: "#FF9A9AA8".into(),
            active_skin: "Default".into(),
            muted_contrast: 1.0,
            primary_font: None,
            secondary_font: None,
            indicator_font: None,
            primary_font_offset: 0,
            secondary_font_offset: 0,
            indicator_font_offset: 0,
            tile_width: 130.0,
            tile_height: 110.0,
            cpu_custom_name: String::new(),
            gpu_custom_name: String::new(),
            selected_disk_mount: "C:".into(),
            network_adapter_name: String::new(),
            // Matches C# AppSettings: Vertical default, Clock first in order.
            orientation: Orientation::Vertical,
            tile_order: vec!["Clock".into(),"CPU".into(),"GPU".into(),"RAM".into(),"Network".into(),"Disk".into()],
            visible_tiles: vec!["CPU".into(),"GPU".into(),"RAM".into(),"Network".into(),"Disk".into()],
            widget_opacity: 0.90,
            click_through: false,
            always_on_top: true,
            round_corners: true,
            update_interval_ms: 1500,
            run_at_startup: false,
            ui_scale: 1.0,
            snap_to_windows: true,
            click_through_hotkey: String::new(),
            network_traffic_indicator: "Off".into(),
            network_arrow_spacing: 16.0,
            arrow_font_offset: 0,
            disk_label_spacing: 16.0,
            disk_label_font_offset: 0,
            disk_label_style: "Letter".into(),
            sync_fonts: true,
            randomize_fonts_on_dice: false,
            window_x: 100.0,
            window_y: 100.0,
            settings_window_x: None,
            settings_window_y: None,
            snap_to_edges: true,
            snap_distance: 20.0,
            game_mode_enabled: false,
            game_mode_hotkey: String::new(),
            game_mode_position: SnapPosition::TopRight,
            game_mode_opacity: 0.7,
            game_mode_orientation: "Current".into(),
            game_mode_click_through: true,
            game_mode_tiles: vec!["CPU".into(),"GPU".into(),"RAM".into()],
            // Matches C#: CPU + GPU temperature warnings @ 85 °C.
            warnings: vec![
                TileWarning { kind: "CPU".into(), enabled: false, metric: WarnMetric::Temperature, threshold: 85.0, flash_enabled: true, flash_color: "#FFFF3333".into(), gradient_mode: false, gradient_color: default_gradient_color() },
                TileWarning { kind: "GPU".into(), enabled: false, metric: WarnMetric::Temperature, threshold: 85.0, flash_enabled: true, flash_color: "#FFFF3333".into(), gradient_mode: true, gradient_color: default_gradient_color() },
            ],
            remote_enabled: false,
            remote_port: 5199,
            remote_key: String::new(),
            remote_devices: Vec::new(),
            snap_blocklist: Vec::new(),
            remote_firewall_configured: false,
            popup_positions: std::collections::HashMap::new(),
            update_check_mode: UpdateMode::Manual,
            last_update_check: None,
            presets: Vec::new(),
            installed_themes: Vec::new(),
            temperature_unit: TempUnit::Celsius,
            start_minimized: false,
            first_run_complete: false,
            cpu_temp_hint_dismissed: false,
            show_remote_status_dot: true,
            cpu_show_temp: true, cpu_show_clock: true,
            gpu_show_temp: true, gpu_show_clock: true, gpu_show_vram: true,
            ram_show_speed: true, ram_show_details: true,
            net_show_down: true, net_show_up: true,
            disk_show_read: true, disk_show_write: true,
            clock_show_date: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Orientation { Vertical, Horizontal }
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SnapPosition {
    TopLeft, TopCenter, TopRight,
    LeftCenter, RightCenter,
    BottomLeft, BottomCenter, BottomRight,
}
impl SnapPosition {
    pub fn from_tag(tag: &str) -> SnapPosition {
        match tag {
            "TopLeft" => SnapPosition::TopLeft,
            "TopCenter" => SnapPosition::TopCenter,
            "TopRight" => SnapPosition::TopRight,
            "LeftCenter" => SnapPosition::LeftCenter,
            "RightCenter" => SnapPosition::RightCenter,
            "BottomLeft" => SnapPosition::BottomLeft,
            "BottomCenter" => SnapPosition::BottomCenter,
            "BottomRight" => SnapPosition::BottomRight,
            _ => SnapPosition::TopRight,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateMode { Auto, Manual, Off }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TempUnit { Celsius, Fahrenheit }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum WarnMetric { #[default] Temperature, Load, UsedGb, Throughput }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TileWarning {
    pub kind: String, pub enabled: bool, pub metric: WarnMetric,
    pub threshold: f64, pub flash_enabled: bool, pub flash_color: String, pub gradient_mode: bool,
    /// The "hot" end of the gradient (the colour the unit shifts toward as the
    /// value approaches the threshold). The cool end is a fixed blue.
    #[serde(default = "default_gradient_color")]
    pub gradient_color: String,
}
fn default_gradient_color() -> String { "#FFFF2200".into() }
impl Default for TileWarning {
    fn default() -> Self { Self { kind: String::new(), enabled: false, metric: WarnMetric::Temperature, threshold: 85.0, flash_enabled: true, flash_color: "#FFFF3333".into(), gradient_mode: false, gradient_color: default_gradient_color() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDevice {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub key: String,
    #[serde(default)]
    pub popout: PopoutSettings,
}

/// Per-device appearance for a remote machine's Popout window. Mirrors the C#
/// PopoutSettings. When `sync_colors` is true the popout uses the widget's own
/// theme; otherwise it uses the colours below.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PopoutSettings {
    pub sync_colors: bool,
    pub bg: String,
    pub tile: String,
    pub accent: String,
    pub text: String,
    pub muted: String,
    pub opacity: f32,
    pub show_cpu: bool,
    pub show_gpu: bool,
    pub show_ram: bool,
    pub show_network: bool,
    pub show_storage: bool,
    pub cpu_label: String,
    pub gpu_label: String,
    /// Per-device tile alerts for this popout (independent of the local widget's).
    #[serde(default)]
    pub warnings: Vec<TileWarning>,
}
impl Default for PopoutSettings {
    fn default() -> Self {
        Self {
            sync_colors: true,
            bg: String::new(), tile: String::new(), accent: String::new(),
            text: String::new(), muted: String::new(),
            opacity: 0.9,
            show_cpu: true, show_gpu: true, show_ram: true,
            show_network: true, show_storage: true,
            cpu_label: String::new(), gpu_label: String::new(),
            warnings: Vec::new(),
        }
    }
}
impl PopoutSettings {
    pub fn warn(&self, kind: &str) -> Option<&TileWarning> { self.warnings.iter().find(|w| w.kind == kind) }
    pub fn warn_mut(&mut self, kind: &str) -> &mut TileWarning {
        if !self.warnings.iter().any(|w| w.kind == kind) { self.warnings.push(TileWarning { kind: kind.to_string(), ..Default::default() }); }
        self.warnings.iter_mut().find(|w| w.kind == kind).unwrap()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSlot { pub name: String, pub bg: String, pub tile: String, pub accent: String, pub text: String, pub muted: String, pub skin: String }

impl AppSettings {
    pub fn warn(&self, kind: &str) -> Option<&TileWarning> { self.warnings.iter().find(|w| w.kind == kind) }
    pub fn warn_mut(&mut self, kind: &str) -> &mut TileWarning {
        if !self.warnings.iter().any(|w| w.kind == kind) { self.warnings.push(TileWarning { kind: kind.to_string(), ..Default::default() }); }
        self.warnings.iter_mut().find(|w| w.kind == kind).unwrap()
    }
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "Fluxid", "Fluxid").map(|d| d.config_dir().to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
    }
    /// Pre-rename config location ("fluidMonitor"), for one-time migration.
    fn legacy_config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "fluidmonitor", "fluidMonitor").map(|d| d.config_dir().to_path_buf())
    }
    /// Copy the old fluidMonitor config into the new Fluxid location once, so a
    /// rename doesn't lose the user's themes, devices, presets, etc.
    fn migrate_legacy() {
        let new_dir = Self::config_dir();
        if new_dir.join("settings.json").exists() { return; }
        if let Some(old_dir) = Self::legacy_config_dir() {
            if old_dir.join("settings.json").exists() {
                let _ = std::fs::create_dir_all(&new_dir);
                if let Ok(entries) = std::fs::read_dir(&old_dir) {
                    for e in entries.flatten() {
                        if e.path().is_file() {
                            let _ = std::fs::copy(e.path(), new_dir.join(e.file_name()));
                        }
                    }
                }
                // Carry over user skins (config_dir/skins/*.json).
                let old_skins = old_dir.join("skins");
                if old_skins.is_dir() {
                    let new_skins = new_dir.join("skins");
                    let _ = std::fs::create_dir_all(&new_skins);
                    if let Ok(es) = std::fs::read_dir(&old_skins) {
                        for e in es.flatten() {
                            if e.path().is_file() {
                                let _ = std::fs::copy(e.path(), new_skins.join(e.file_name()));
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn config_path() -> PathBuf { Self::config_dir().join("settings.json") }
    pub fn load() -> Result<Self> {
        Self::migrate_legacy();
        let path = Self::config_path();
        if !path.exists() { return Ok(Self::default()); }
        let json = std::fs::read_to_string(&path)?;
        match serde_json::from_str(&json) {
            Ok(s) => Ok(s),
            Err(e) => {
                // Corrupt / unparseable config: preserve it as a .bak before
                // falling back to defaults, so the next save doesn't silently
                // overwrite (and permanently destroy) the user's settings — and
                // they can recover by hand if they want to.
                let bak = path.with_extension("json.bak");
                let _ = std::fs::rename(&path, &bak);
                eprintln!(
                    "fluid-core: settings.json was unreadable ({e}); backed up to {} and reset to defaults",
                    bak.display()
                );
                Ok(Self::default())
            }
        }
    }
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let json = serde_json::to_string_pretty(self)?;
        // Atomic write: serialize to a sibling temp file, then rename over the
        // real file. A crash/kill mid-write can then never leave a truncated or
        // half-written settings.json (which would reset every setting on the
        // next launch). rename() within the same directory is atomic on Windows
        // and Unix, and replaces the existing file.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}
