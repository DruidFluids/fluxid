//! Fluxid widget — the iced daemon: windows, update loop, snapping,
//! game mode, hotkeys, remote monitoring, and the system tray.

mod tile;
mod style;
mod fmt;
mod settings_panel;
mod popups;
mod fonts;
mod hotkeys;
mod updates;
mod firewall;
mod cpu_driver;

use fluid_core::sensor_data::SensorSnapshot;
use fluid_core::settings::{AppSettings, Orientation, SnapPosition, TempUnit, WarnMetric};
use fluid_sensor::SensorPoller;
use iced::widget::{button, column, container, mouse_area, row, text, Space};
use iced::{window, Border, Color, Element, Length, Point, Size, Subscription, Task, Theme};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};
use style::Palette;
use tile::WarnView;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder,
};

// Unique title applied to the widget window so click-through targets only it
// (the iced daemon otherwise gives every window the same title).
const WIDGET_TITLE: &str = "Fluxid Widget";
// The default window title the iced daemon assigns a new window before it's
// registered in our state (App::title's fallback). widget_hwnd() finds the
// window by this title, then renames it to WIDGET_TITLE. Keep the two in sync.
const DEFAULT_TITLE: &str = "Fluxid";

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut app = iced::daemon(App::title, App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme);
    // Load Segoe UI Symbol so the monochrome icon glyphs (die, folder, moon,
    // sun, undo) render — the same font the C# app uses for these icons.
    #[cfg(target_os = "windows")]
    {
        if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\seguisym.ttf") {
            app = app.font(bytes);
        }
    }
    app.run_with(App::new)
}

fn make_tray_icon() -> tray_icon::Icon {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let corner = 6i32;
            let (xi, yi, s) = (x as i32, y as i32, SIZE as i32);
            let in_corner = (xi < corner && yi < corner && (corner - xi).pow(2) + (corner - yi).pow(2) > corner.pow(2))
                || (xi >= s - corner && yi < corner && (xi - (s - corner)).pow(2) + (corner - yi).pow(2) > corner.pow(2))
                || (xi < corner && yi >= s - corner && (corner - xi).pow(2) + (yi - (s - corner)).pow(2) > corner.pow(2))
                || (xi >= s - corner && yi >= s - corner && (xi - (s - corner)).pow(2) + (yi - (s - corner)).pow(2) > corner.pow(2));
            if in_corner { rgba.extend_from_slice(&[0,0,0,0]); }
            else { rgba.extend_from_slice(&[0,168,255,255]); }
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("tray icon")
}

#[cfg(target_os = "windows")]
fn work_area() -> Option<(f32, f32, f32, f32)> {
    // Work area of the monitor the widget is actually on, returned in LOGICAL
    // coordinates (divided by that monitor's DPI scale) so it matches the
    // logical window positions iced reports. SPI_GETWORKAREA only covered the
    // primary monitor in physical pixels — wrong on scaled/multi-monitor setups.
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    let hwnd = widget_hwnd()?;
    unsafe {
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return None;
        }
        let dpi = GetDpiForWindow(hwnd);
        let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
        let w = mi.rcWork;
        Some((
            w.left as f32 / scale,
            w.top as f32 / scale,
            w.right as f32 / scale,
            w.bottom as f32 / scale,
        ))
    }
}
#[cfg(not(target_os = "windows"))]
fn work_area() -> Option<(f32, f32, f32, f32)> { None }

// Rects (logical coords) of all visible top-level app windows (excluding the
// widget itself) so the widget can dock to any window's outer edges.
#[cfg(target_os = "windows")]
fn own_window_rects(blocklist: &[String]) -> Vec<(f32, f32, f32, f32)> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindowVisible,
    };

    let widget = widget_hwnd();
    let scale = match widget {
        Some(h) => { let d = unsafe { GetDpiForWindow(h) }; if d == 0 { 1.0 } else { d as f32 / 96.0 } }
        None => 1.0,
    };
    // Lowercased blocklist for case-insensitive substring matching.
    let block: Vec<String> = blocklist.iter().map(|s| s.to_lowercase()).collect();
    struct Ctx { widget: isize, scale: f32, block: Vec<String>, rects: Vec<(f32, f32, f32, f32)> }
    let mut ctx = Ctx {
        widget: widget.map(|h| h.0 as isize).unwrap_or(0),
        scale,
        block,
        rects: Vec::new(),
    };

    unsafe extern "system" fn cb(h: HWND, lp: LPARAM) -> BOOL {
        let ctx = &mut *(lp.0 as *mut Ctx);
        // Visible, not minimized, has a title (skips tool/helper windows), and
        // not the widget itself.
        if h.0 as isize != ctx.widget
            && IsWindowVisible(h).as_bool() && !IsIconic(h).as_bool()
            && GetWindowTextLengthW(h) > 0
        {
            // Skip windows whose title matches any blocklist rule.
            if !ctx.block.is_empty() {
                let mut buf = [0u16; 256];
                let n = GetWindowTextW(h, &mut buf);
                if n > 0 {
                    let title = String::from_utf16_lossy(&buf[..n as usize]).to_lowercase();
                    if ctx.block.iter().any(|b| !b.is_empty() && title.contains(b.as_str())) {
                        return BOOL(1);
                    }
                }
            }
            let mut r = RECT::default();
            if GetWindowRect(h, &mut r).is_ok() {
                let w = (r.right - r.left) as f32;
                let hgt = (r.bottom - r.top) as f32;
                if w > 120.0 && hgt > 120.0 {
                    let s = ctx.scale;
                    ctx.rects.push((r.left as f32 / s, r.top as f32 / s, r.right as f32 / s, r.bottom as f32 / s));
                }
            }
        }
        BOOL(1)
    }
    unsafe { let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize)); }
    ctx.rects
}
#[cfg(not(target_os = "windows"))]
fn own_window_rects(_blocklist: &[String]) -> Vec<(f32, f32, f32, f32)> { Vec::new() }

// Open a URL in the user's default browser (non-elevated). Used by the
// Utilities window instead of executing remote scripts.
#[cfg(target_os = "windows")]
fn open_url(url: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let to_w = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let op = to_w("open");
    let u = to_w(url);
    unsafe {
        ShellExecuteW(None, PCWSTR(op.as_ptr()), PCWSTR(u.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}
#[cfg(not(target_os = "windows"))]
fn open_url(_url: &str) {}

// Titles of visible top-level windows (for the snap-blocklist Pick Window list).
#[cfg(target_os = "windows")]
fn enum_window_titles() -> Vec<String> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindowVisible,
    };
    struct Ctx { widget: isize, titles: Vec<String> }
    let mut ctx = Ctx { widget: widget_hwnd().map(|h| h.0 as isize).unwrap_or(0), titles: Vec::new() };
    unsafe extern "system" fn cb(h: HWND, lp: LPARAM) -> BOOL {
        let ctx = &mut *(lp.0 as *mut Ctx);
        if h.0 as isize != ctx.widget
            && IsWindowVisible(h).as_bool() && !IsIconic(h).as_bool()
            && GetWindowTextLengthW(h) > 0
        {
            let mut buf = [0u16; 256];
            let n = GetWindowTextW(h, &mut buf);
            if n > 0 {
                let t = String::from_utf16_lossy(&buf[..n as usize]);
                if !t.starts_with("Fluxid") && !ctx.titles.contains(&t) { ctx.titles.push(t); }
            }
        }
        BOOL(1)
    }
    unsafe { let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize)); }
    ctx.titles
}
#[cfg(not(target_os = "windows"))]
fn enum_window_titles() -> Vec<String> { Vec::new() }

#[cfg(target_os = "windows")]
fn set_run_at_startup(on: bool) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok((key, _)) = hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        if on { if let Ok(exe) = std::env::current_exe() { let _ = key.set_value("Fluxid", &exe.to_string_lossy().to_string()); } }
        else { let _ = key.delete_value("Fluxid"); let _ = key.delete_value("fluidMonitor"); }
    }
}
#[cfg(not(target_os = "windows"))]
fn set_run_at_startup(_: bool) {}

// iced/winit doesn't expose raw HWND access. The daemon title fn runs before the
// window is registered in our state, so the widget keeps the default
// "Fluxid" title. We resolve the widget HWND once (it's the only such
// window at startup), rename it to a unique title, and cache the handle so later
// lookups never depend on the title again.
#[cfg(target_os = "windows")]
fn widget_hwnd() -> Option<windows::Win32::Foundation::HWND> {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::core::HSTRING;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetWindowTextW};
    static CACHED: AtomicIsize = AtomicIsize::new(0);
    let cached = CACHED.load(Ordering::Relaxed);
    if cached != 0 {
        return Some(HWND(cached as *mut _));
    }
    unsafe {
        if let Ok(h) = FindWindowW(None, &HSTRING::from(WIDGET_TITLE)) {
            if !h.0.is_null() {
                CACHED.store(h.0 as isize, Ordering::Relaxed);
                return Some(h);
            }
        }
        if let Ok(h) = FindWindowW(None, &HSTRING::from(DEFAULT_TITLE)) {
            if !h.0.is_null() {
                let _ = SetWindowTextW(h, &HSTRING::from(WIDGET_TITLE));
                CACHED.store(h.0 as isize, Ordering::Relaxed);
                return Some(h);
            }
        }
    }
    None
}

// Toggle WS_EX_TRANSPARENT (click-through) + WS_EX_LAYERED on the widget window.
#[cfg(target_os = "windows")]
fn set_click_through(_title: &str, on: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };
    let hwnd = match widget_hwnd() {
        Some(h) => h,
        None => return,
    };
    unsafe {
        let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let flags = (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as isize;
        if on { ex |= flags; } else { ex &= !flags; }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
    }
}
#[cfg(not(target_os = "windows"))]
fn set_click_through(_: &str, _: bool) {}

// Resolve + cache the widget HWND (renames the window to a unique title).
fn rename_widget_window() {
    #[cfg(target_os = "windows")]
    { let _ = widget_hwnd(); }
}

// Current mouse cursor position in logical (DPI-scaled) screen coordinates,
// matching the coordinate space iced uses for window positioning. iced's
// right-press event doesn't expose the cursor, so we read it from Win32.
#[cfg(target_os = "windows")]
fn cursor_logical_pos() -> Option<(f32, f32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        // Convert physical pixels -> logical points via the widget's DPI.
        let scale = match widget_hwnd() {
            Some(h) => {
                let dpi = GetDpiForWindow(h);
                if dpi == 0 { 96.0 } else { dpi as f32 }
            }
            None => 96.0,
        } / 96.0;
        Some((pt.x as f32 / scale, pt.y as f32 / scale))
    }
}
#[cfg(not(target_os = "windows"))]
fn cursor_logical_pos() -> Option<(f32, f32)> { None }

#[derive(Debug, Clone, Copy, PartialEq)]
enum WindowKind { Widget, Settings, Alerts, GameMode, Help, WidgetMenu, Popout, Utilities, WindowPicker, ThemeStore, PopoutConfig, CpuDriver, Picker, ConfirmDelete }

// Settings window is a single FIXED size for every tab — it never grows or
// shrinks when switching tabs. The height is sized to the tallest tab
// (Appearance: colors + size + fonts) so nothing is clipped; shorter tabs
// simply have empty space below. The hidden scrollbar catches any slight
// overflow without ever showing a bar.
const SETTINGS_FIXED_SIZE: Size = Size::new(560.0, 720.0);
fn settings_size_for_tab(_tab: usize) -> Size {
    SETTINGS_FIXED_SIZE
}

// Keep secondary windows (settings, popups, menus) off the taskbar so only the
// widget shows a single entry.
fn no_taskbar() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific { skip_taskbar: true, ..Default::default() }
}

// Persisted-position key for a popup kind. Widget/Settings keep dedicated
// fields; the right-click WidgetMenu always opens at the cursor.
fn kind_key(kind: WindowKind) -> Option<&'static str> {
    match kind {
        WindowKind::Alerts => Some("alerts"),
        WindowKind::GameMode => Some("gamemode"),
        WindowKind::Help => Some("help"),
        WindowKind::Utilities => Some("utilities"),
        WindowKind::WindowPicker => Some("windowpicker"),
        WindowKind::ThemeStore => Some("themestore"),
        WindowKind::PopoutConfig => Some("popoutconfig"),
        WindowKind::CpuDriver => Some("cpudriver"),
        WindowKind::Picker => Some("picker"),
        WindowKind::ConfirmDelete => Some("confirmdelete"),
        WindowKind::Popout => Some("popout"),
        WindowKind::Widget | WindowKind::Settings | WindowKind::WidgetMenu => None,
    }
}

// Which panel the optional-CPU-driver (PawnIO) dialog is showing. Mirrors the
// C# CpuTempDialog panels: Primary (pitch / manage), Info (links), Progress,
// and Done (success or failed-with-fallback).
#[derive(Debug, Clone)]
enum CpuDriverStage {
    Primary,
    Info,
    Progress(String),
    Done { ok: bool, title: String, body: String, show_fallback: bool },
}

// Snapshot of all appearance state for the C# "Undo last change" stack
// (colors + skin + fonts). Up to 5 steps back.
#[derive(Clone)]
struct Appearance {
    bg: String, tile: String, accent: String, text: String, muted: String,
    skin: String,
    primary_font: Option<String>, secondary_font: Option<String>, indicator_font: Option<String>,
}

fn nanos() -> usize {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize).unwrap_or(0)
}

// Stable per-device id (time + monotonic counter, hex). Used to key remote
// device connections and popout windows.
fn new_device_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64).unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:x}", t, n)
}

// Evaluate one tile alert against a snapshot, returning the flash/colour view.
// Shared by the local widget's per-device popouts (which keep their own alert
// set). `flash_on` is the global blink phase so the tile pulses rather than
// staying lit.
fn warn_view_for(warnings: &[fluid_core::settings::TileWarning], kind: &str, snap: &SensorSnapshot, flash_on: bool) -> WarnView {
    let w = match warnings.iter().find(|w| w.kind == kind) {
        Some(w) if w.enabled => w,
        _ => return WarnView::default(),
    };
    let (temp, load, used_gb): (Option<f32>, f32, f32) = match kind {
        "CPU" => (snap.cpu.temperature_c, snap.cpu.usage_percent, 0.0),
        "GPU" => (snap.gpu.temperature_c, snap.gpu.usage_percent, 0.0),
        "RAM" => (None, snap.ram.usage_percent, snap.ram.used_mb / 1024.0),
        _ => return WarnView::default(),
    };
    let current: f64 = match w.metric {
        WarnMetric::Temperature => temp.unwrap_or(0.0) as f64,
        WarnMetric::Load => load as f64,
        WarnMetric::UsedGb => used_gb as f64,
        WarnMetric::Throughput => 0.0,
    };
    let exceeded = current >= w.threshold;
    let accent_override = if w.gradient_mode && w.metric == WarnMetric::Temperature {
        let hot = style::parse_hex(&w.gradient_color, Color::from_rgb(1.0, 0.13, 0.0));
        temp.and_then(|t| { let dist = w.threshold - t as f64; if dist <= 15.0 { Some(style::gradient_color(dist, hot)) } else { None } })
    } else { None };
    WarnView { flash: exceeded && w.flash_enabled && flash_on, accent_override }
}

// A small glowing connection-status dot: a bright core inside a soft same-colour
// halo. Green = connected, red = disconnected.
fn status_dot<'a>(c: Color) -> Element<'a, Message> {
    container(
        container(Space::new(6, 6)).style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(c)),
            border: Border { radius: 3.0.into(), ..Border::default() },
            ..Default::default()
        })
    )
    .padding(2)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color { a: 0.35, ..c })),
        border: Border { radius: 5.0.into(), ..Border::default() },
        ..Default::default()
    })
    .into()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}\u{2026}")
    }
}

struct App {
    settings: AppSettings,
    snapshot: SensorSnapshot,
    poller: Option<SensorPoller>,
    windows: BTreeMap<window::Id, WindowKind>,
    warn_state: HashMap<String, (bool, Option<Color>)>,
    flash_on: bool,
    anim_t: f32,
    font_list: Vec<String>,
    appearance_undo: Vec<Appearance>,
    editing_color: Option<u8>,
    settings_tab: usize,
    game_mode: bool,
    click_through_applied: bool,
    pending_snap: Option<(window::Id, Point, Instant)>,
    ignore_next_move: bool,
    // When snapped to the right/bottom edge, resizes keep that edge anchored so
    // the widget grows away from the corner.
    snap_right: bool,
    snap_bottom: bool,
    _tray: TrayIcon,
    settings_id: tray_icon::menu::MenuId,
    show_id: tray_icon::menu::MenuId,
    game_id: tray_icon::menu::MenuId,
    exit_id: tray_icon::menu::MenuId,
    // ── Remote monitoring ──
    remote: Option<fluid_remote::RemoteManager>,
    remote_rx: Option<std::sync::mpsc::Receiver<fluid_remote::RemoteEvent>>,
    remote_snapshots: HashMap<String, SensorSnapshot>,
    remote_conn: HashMap<String, bool>,
    popout_device: HashMap<window::Id, String>,
    pending_popout: std::collections::VecDeque<String>,
    remote_expanded: bool,
    // ── Global hotkeys ──
    hotkeys: Option<hotkeys::HotkeyManager>,
    hotkey_rx: Option<std::sync::mpsc::Receiver<hotkeys::HotkeyEvent>>,
    capturing_hotkey: Option<hotkeys::HotkeyTarget>,
    // ── Utilities (Tweaks) ──
    blocklist_editor: iced::widget::text_editor::Content,
    blocklist_status: String,
    // ── Updates ──
    update_checking: bool,
    update_status: String,
    update_status_kind: u8, // 0 neutral, 1 good (green), 2 bad (red)
    update_available: Option<updates::PendingUpdate>,
    appearance_status: String,
    theme_store_franchise: Option<usize>,
    config_device: Option<String>,
    add_device_open: bool,
    new_device_name: String,
    new_device_ip: String,
    new_device_key: String,
    device_test_status: String,
    device_test_ok: bool,
    // ── Optional CPU sensor driver (PawnIO) ──
    cpu_driver_installed: bool,
    cpu_dialog: CpuDriverStage,
    // Which device the widget is showing: None = this PC, Some(id) = a remote.
    widget_device: Option<String>,
    // Which tile's section is expanded in the Tiles settings tab (accordion).
    tiles_section: Option<String>,
    // Saved-Themes slot armed for save (click number -> save icon -> click to save).
    preset_arming: Option<u8>,
    // Picker popup mode: false = themes, true = skins.
    picker_skins: bool,
    // Saved-Themes slot pending delete confirmation.
    confirm_delete_slot: Option<u8>,
}

#[derive(Debug, Clone)]
enum Message {
    SensorTick, TrayPoll, FlashTick, AnimTick,
    DragWindow(window::Id),
    WindowOpened(window::Id, WindowKind),
    WindowClosed(window::Id),
    WindowMoved(window::Id, Point),
    OpenSettings, HideWidget, SaveClose, ResetDefaults, Noop,
    OpenAlerts, OpenGameMode, OpenHelp, OpenUtilities, ClosePopup(window::Id),
    OpenUrl(String),
    OpenThemeStore, ApplyPackTheme(usize, usize),
    ThemeStoreOpenFranchise(usize), ThemeStoreBack,
    BlocklistAction(iced::widget::text_editor::Action), SaveBlocklist,
    PickWindow, PickWindowChosen(String),
    ShowWidgetMenu, WidgetMenuSettings, WidgetMenuExit, WindowUnfocused(window::Id),
    ToggleTile(String, bool),
    SetOpacity(f32), SetOrientation(Orientation),
    SetFahrenheit(bool), SetSnap(bool),
    ThemePrev, ThemeNext, SetColorMode(bool),
    SetWarnEnabled(String, bool),
    SetWarnFlash(String, bool), SetWarnGradient(String, bool),
    SetWarnMetric(String, WarnMetric), SetWarnThresholdStr(String, String), SetWarnFlashColor(String, String),
    SetWarnGradientColor(String, String),
    SetHexColor(u8, String),
    SetTileWidth(f32), SetTileHeight(f32),
    SetPrimaryFontOffset(f32), SetSecondaryFontOffset(f32), SetIndicatorFontOffset(f32),
    SetMutedContrast(f32), SetInterval(f32),
    SetCpuName(String), SetGpuName(String),
    SetDisk(String), SetAdapter(String),
    SetAlwaysOnTop(bool), SetRunAtStartup(bool),
    SetUiScale(f32), SetClickThrough(bool), SetSnapWindows(bool), SetSnapDistance(f32),
    SnapWidgetNow,
    TrafficCycle,
    SetArrowSpacing(f32), SetArrowFontOffset(f32),
    SetDiskLabelSpacing(f32), SetDiskLabelFontOffset(f32),
    DiskLabelCycle,
    SkinPrev, SkinNext,
    RandomizeAppearance, RandomizeSkinOnly, UndoAppearance,
    SetSyncFonts(bool), SetRandomizeFonts(bool),
    SetFont(u8, String),
    SetUpdateMode(String),
    ExportAppearance, ImportAppearance, ImportAppearanceCode(Option<String>),
    CheckForUpdates,
    UpdateCheckDone(updates::CheckResult),
    DownloadUpdate,
    UpdateDownloadDone(Result<(), String>),
    UpdateLater,
    PresetSlotClick(u8),
    OpenThemePicker, OpenSkinPicker, ApplyThemePreset(usize), ApplySkin(String),
    ConfirmDeletePreset(u8), DeletePresetConfirmed,
    EditColor(u8),
    SetSettingsTab(usize),
    ArmHotkey(hotkeys::HotkeyTarget),
    HotkeyKeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),
    ClearHotkey(hotkeys::HotkeyTarget),
    RemotePoll,
    ToggleRemoteSection(bool),
    SetTcpFeedEnabled(bool),
    CopyHandshakeKey,
    RegenerateKey,
    ShowAddDevice, CancelAddDevice,
    SetNewDeviceName(String), SetNewDeviceIp(String), SetNewDeviceKey(String),
    TestDevice, SaveDevice, RemoveDevice(String),
    OpenPopout(String),
    OpenPopoutConfig(String),
    PopoutSyncColors(String, bool), PopoutColor(String, u8, String), PopoutOpacity(String, f32),
    PopoutTile(String, String, bool), PopoutLabel(String, u8, String),
    PopoutWarnEnabled(String, String, bool), PopoutWarnMetric(String, String, WarnMetric),
    PopoutWarnThreshold(String, String, String), PopoutWarnFlash(String, String, bool),
    PopoutWarnFlashColor(String, String, String), PopoutWarnGradient(String, String, bool), PopoutWarnGradientColor(String, String, String),
    SetGameModeEnabled(bool),
    SetGameModePosition(SnapPosition), SetGameModeOpacity(f32),
    SetGameModeOrientation(String), SetGameModeClickThrough(bool),
    ToggleGameModeTile(String, bool),
    // ── Optional CPU sensor driver (PawnIO) ──
    OpenCpuDriver, DismissCpuTempHint,
    SwitchWidgetDevice(Option<String>), SetShowRemoteStatusDot(bool),
    ToggleTileSection(String), SetTileField(String, bool),
    CpuDriverMoreInfo, CpuDriverBack,
    CpuDriverInstall, CpuDriverUninstall,
    CpuDriverInstallDone(cpu_driver::Outcome),
    CpuDriverUninstallDone(cpu_driver::Outcome),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let mut settings = AppSettings::load().unwrap_or_default();
        // Assign stable ids to any devices loaded from an older config.
        let mut devices_changed = false;
        for d in settings.remote_devices.iter_mut() {
            if d.id.is_empty() { d.id = new_device_id(); devices_changed = true; }
        }
        if devices_changed { let _ = settings.save(); }

        // Start the remote-monitoring runtime; it hands back the handshake key.
        let (remote, remote_rx, handshake_key) =
            fluid_remote::RemoteManager::start(settings.remote_port);
        settings.remote_key = handshake_key;
        remote.set_devices(settings.remote_devices.clone());
        if settings.remote_enabled { remote.set_server_enabled(true); }
        let remote_expanded = settings.remote_enabled;

        // Register global hotkeys from the saved combos.
        let (hotkeys_mgr, hotkey_rx) = hotkeys::HotkeyManager::start();
        hotkeys_mgr.set_combo(hotkeys::HotkeyTarget::ClickThrough, &settings.click_through_hotkey);
        hotkeys_mgr.set_combo(hotkeys::HotkeyTarget::GameMode, &settings.game_mode_hotkey);

        let blocklist_text = settings.snap_blocklist.join("\n");

        let menu = Menu::new();
        let si = MenuItem::new("Settings", true, None);
        let wi = MenuItem::new("Show Widget", true, None);
        let gi = MenuItem::new("Game Mode", true, None);
        let ei = MenuItem::new("Exit", true, None);
        let (sid, wid, gid, eid) = (si.id().clone(), wi.id().clone(), gi.id().clone(), ei.id().clone());
        menu.append(&si).ok(); menu.append(&wi).ok(); menu.append(&gi).ok(); menu.append(&ei).ok();
        let tray = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Fluxid").with_icon(make_tray_icon()).build().expect("tray");
        let app = Self {
            settings, snapshot: SensorSnapshot::default(), poller: None,
            windows: BTreeMap::new(), warn_state: HashMap::new(),
            flash_on: false, anim_t: 0.0, font_list: fonts::system_fonts(), appearance_undo: Vec::new(), editing_color: None, settings_tab: 0, game_mode: false,
            click_through_applied: false,
            pending_snap: None, ignore_next_move: false, snap_right: false, snap_bottom: false,
            _tray: tray, settings_id: sid, show_id: wid, game_id: gid, exit_id: eid,
            remote: Some(remote), remote_rx: Some(remote_rx),
            remote_snapshots: HashMap::new(), remote_conn: HashMap::new(),
            popout_device: HashMap::new(), pending_popout: std::collections::VecDeque::new(), remote_expanded,
            hotkeys: Some(hotkeys_mgr), hotkey_rx: Some(hotkey_rx), capturing_hotkey: None,
            blocklist_editor: iced::widget::text_editor::Content::with_text(&blocklist_text),
            blocklist_status: String::new(),
            update_checking: false, update_status: String::new(), update_status_kind: 0, update_available: None,
            appearance_status: String::new(),
            theme_store_franchise: None,
            config_device: None,
            add_device_open: false,
            new_device_name: String::new(), new_device_ip: String::new(), new_device_key: String::new(),
            device_test_status: String::new(), device_test_ok: false,
            cpu_driver_installed: cpu_driver::is_installed(),
            cpu_dialog: CpuDriverStage::Primary,
            widget_device: None,
            tiles_section: None,
            preset_arming: None,
            picker_skins: false,
            confirm_delete_slot: None,
        };
        let size = app.widget_size();
        let position = if app.settings.first_run_complete {
            window::Position::Specific(Point::new(app.settings.window_x as f32, app.settings.window_y as f32))
        } else { window::Position::Centered };
        let level = if app.settings.always_on_top { window::Level::AlwaysOnTop } else { window::Level::Normal };
        let (_id, open) = window::open(window::Settings {
            size, position, decorations: false, transparent: true, resizable: false, level, ..Default::default()
        });
        let open_task = open.map(|id| Message::WindowOpened(id, WindowKind::Widget));
        // Auto mode: silently check for updates on launch.
        let task = if app.settings.update_check_mode == fluid_core::settings::UpdateMode::Auto {
            Task::batch([open_task, Task::done(Message::CheckForUpdates)])
        } else {
            open_task
        };
        (app, task)
    }

    fn effective_orientation(&self) -> Orientation {
        if self.game_mode {
            match self.settings.game_mode_orientation.as_str() {
                "Horizontal" => Orientation::Horizontal,
                "Vertical" => Orientation::Vertical,
                _ => self.settings.orientation.clone(),
            }
        } else {
            self.settings.orientation.clone()
        }
    }

    fn current_tiles(&self) -> Vec<String> {
        // Canonical C# order (AppSettings.TileOrder default): Clock, CPU, GPU,
        // RAM, Network, Storage. The Rust port has no drag-reorder UI, so we
        // always render in this order, filtered by which tiles are enabled.
        const CANONICAL: [&str; 6] = ["Clock", "CPU", "GPU", "RAM", "Network", "Disk"];
        let enabled = if self.game_mode { &self.settings.game_mode_tiles } else { &self.settings.visible_tiles };
        CANONICAL.iter()
            .filter(|t| enabled.iter().any(|v| v == *t))
            .map(|s| s.to_string())
            .collect()
    }
    fn widget_size(&self) -> Size {
        let n = self.current_tiles().len().max(1) as f32;
        let sc = self.settings.ui_scale;
        let tw = self.settings.tile_width * sc;
        let th = self.settings.tile_height * sc;
        let sp = style::skin_style(&self.settings.active_skin).tile_spacing;
        // The device switcher tabs add a row above the tiles (only when a remote
        // device exists and we're not in the compact game-mode overlay).
        let tabs_h = if self.show_device_tabs() { 24.0 } else { 0.0 };
        match self.effective_orientation() {
            Orientation::Horizontal => Size::new(16.0 + n * tw + (n - 1.0) * sp, 8.0 + 20.0 + 4.0 + tabs_h + th + 8.0),
            Orientation::Vertical => Size::new(tw + 16.0, 8.0 + 20.0 + 4.0 + tabs_h + n * th + (n - 1.0) * sp + 8.0),
        }
    }
    // The device switcher shows only when at least one remote device exists and
    // the widget isn't in game-mode (a compact local-only overlay).
    fn show_device_tabs(&self) -> bool {
        !self.game_mode && !self.settings.remote_devices.is_empty()
    }
    fn widget_window(&self) -> Option<window::Id> {
        self.windows.iter().find(|(_, k)| **k == WindowKind::Widget).map(|(id, _)| *id)
    }
    // Keep settings/popups above the always-on-top widget: drop the widget to
    // Normal level while any other window is open, restore when none remain.
    fn update_widget_level(&self) -> Task<Message> {
        let others_open = self.windows.values().any(|k| *k != WindowKind::Widget && *k != WindowKind::WidgetMenu);
        let level = if self.settings.always_on_top && !others_open {
            window::Level::AlwaysOnTop
        } else {
            window::Level::Normal
        };
        self.widget_window().map(|id| window::change_level(id, level)).unwrap_or(Task::none())
    }
    fn settings_window(&self) -> Option<window::Id> {
        self.windows.iter().find(|(_, k)| **k == WindowKind::Settings).map(|(id, _)| *id)
    }
    fn open_settings(&mut self) -> Task<Message> {
        if self.settings_window().is_some() { return Task::none(); }
        let pos = match (self.settings.settings_window_x, self.settings.settings_window_y) {
            (Some(x), Some(y)) => window::Position::Specific(Point::new(x as f32, y as f32)),
            _ => window::Position::Default,
        };
        let (_, t) = window::open(window::Settings {
            size: settings_size_for_tab(self.settings_tab), position: pos, decorations: false, transparent: true, resizable: false,
            level: window::Level::AlwaysOnTop, platform_specific: no_taskbar(), ..Default::default()
        });
        t.map(|id| Message::WindowOpened(id, WindowKind::Settings))
    }
    fn open_popup(&self, kind: WindowKind, size: Size) -> Task<Message> {
        if self.windows.values().any(|k| *k == kind) { return Task::none(); }
        let (_, t) = window::open(window::Settings {
            size, position: self.popup_position(kind), decorations: false, transparent: true,
            resizable: false, level: window::Level::AlwaysOnTop, platform_specific: no_taskbar(), ..Default::default()
        });
        t.map(move |id| Message::WindowOpened(id, kind))
    }
    // Remembered position for a popup kind, falling back to centered.
    fn popup_position(&self, kind: WindowKind) -> window::Position {
        kind_key(kind)
            .and_then(|k| self.settings.popup_positions.get(k))
            .map(|(x, y)| window::Position::Specific(Point::new(*x as f32, *y as f32)))
            .unwrap_or(window::Position::Centered)
    }
    fn close_kind(&self, kind: WindowKind) -> Task<Message> {
        let ids: Vec<_> = self.windows.iter().filter(|(_, k)| **k == kind).map(|(id, _)| *id).collect();
        Task::batch(ids.into_iter().map(window::close))
    }
    fn resize_widget(&mut self) -> Task<Message> {
        let id = match self.widget_window() { Some(i) => i, None => return Task::none() };
        let sz = self.widget_size();
        let mut tasks = vec![window::resize(id, sz)];
        // Keep the snapped corner anchored so the widget grows away from it.
        if (self.snap_right || self.snap_bottom) && !self.game_mode {
            if let Some((_, t, r, b)) = work_area() {
                let x = if self.snap_right { r - sz.width } else { self.settings.window_x as f32 };
                let y = if self.snap_bottom { b - sz.height } else { self.settings.window_y as f32 };
                let _ = t;
                self.settings.window_x = x as f64;
                self.settings.window_y = y as f64;
                self.ignore_next_move = true;
                tasks.push(window::move_to(id, Point::new(x, y)));
                let _ = self.settings.save();
            }
        }
        Task::batch(tasks)
    }
    // ── Hotkey helpers ──
    fn set_hotkey(&mut self, target: hotkeys::HotkeyTarget, combo: String) {
        match target {
            hotkeys::HotkeyTarget::ClickThrough => self.settings.click_through_hotkey = combo.clone(),
            hotkeys::HotkeyTarget::GameMode => self.settings.game_mode_hotkey = combo.clone(),
        }
        let _ = self.settings.save();
        if let Some(h) = &self.hotkeys { h.set_combo(target, &combo); }
    }
    fn toggle_click_through(&mut self) -> Task<Message> {
        self.settings.click_through = !self.settings.click_through;
        let _ = self.settings.save();
        self.apply_click_through()
    }
    fn toggle_game_mode(&mut self) -> Task<Message> {
        // Don't enter game mode if it isn't enabled in settings (matches C#
        // EnterGameMode); exiting always works.
        if !self.game_mode && !self.settings.game_mode_enabled { return Task::none(); }
        self.game_mode = !self.game_mode;
        let mut tasks: Vec<Task<Message>> = Vec::new();
        if let Some(id) = self.widget_window() {
            tasks.push(window::resize(id, self.widget_size()));
            if self.game_mode {
                if let Some(c) = self.game_corner() { self.ignore_next_move = true; tasks.push(window::move_to(id, c)); }
            } else {
                self.ignore_next_move = true;
                tasks.push(window::move_to(id, Point::new(self.settings.window_x as f32, self.settings.window_y as f32)));
            }
        }
        tasks.push(self.apply_click_through());
        Task::batch(tasks)
    }
    fn drain_hotkey_events(&mut self) -> Task<Message> {
        let mut events = Vec::new();
        if let Some(rx) = &self.hotkey_rx { while let Ok(e) = rx.try_recv() { events.push(e); } }
        let mut tasks: Vec<Task<Message>> = Vec::new();
        for e in events {
            match e {
                hotkeys::HotkeyEvent::ClickThrough => tasks.push(self.toggle_click_through()),
                hotkeys::HotkeyEvent::GameMode => tasks.push(self.toggle_game_mode()),
            }
        }
        if tasks.is_empty() { Task::none() } else { Task::batch(tasks) }
    }

    // ── Updates helper ──
    fn last_checked_label(&self) -> String {
        match &self.settings.last_update_check {
            None => "Last checked: never".into(),
            Some(raw) => match chrono::DateTime::parse_from_rfc3339(raw) {
                Ok(dt) => {
                    let ago = chrono::Local::now().signed_duration_since(dt);
                    let mins = ago.num_minutes();
                    if mins < 1 { "Last checked: just now".into() }
                    else if mins < 60 { format!("Last checked: {mins} min ago") }
                    else if ago.num_hours() < 24 { format!("Last checked: {}h ago", ago.num_hours()) }
                    else { format!("Last checked: {}", dt.format("%b %d")) }
                }
                Err(_) => "Last checked: never".into(),
            },
        }
    }

    // ── Remote monitoring helpers ──
    fn drain_remote_events(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.remote_rx {
            while let Ok(ev) = rx.try_recv() { events.push(ev); }
        }
        for ev in events {
            match ev {
                fluid_remote::RemoteEvent::KeyChanged(k) => { self.settings.remote_key = k; }
                fluid_remote::RemoteEvent::ConnState { device_id, connected } => {
                    self.remote_conn.insert(device_id, connected);
                }
                fluid_remote::RemoteEvent::Snapshot { device_id, snapshot } => {
                    self.remote_snapshots.insert(device_id, *snapshot);
                }
                fluid_remote::RemoteEvent::TestResult { ok, message } => {
                    self.device_test_ok = ok;
                    self.device_test_status = if ok { "\u{2713} Connected".into() }
                        else { format!("\u{2717} {message}") };
                }
            }
        }
    }
    fn device_mut(&mut self, id: &str) -> Option<&mut fluid_core::settings::RemoteDevice> {
        self.settings.remote_devices.iter_mut().find(|d| d.id == id)
    }
    fn build_device_from_form(&self) -> Option<fluid_core::settings::RemoteDevice> {
        let name = self.new_device_name.trim();
        let ip = self.new_device_ip.trim();
        let key = self.new_device_key.trim();
        if name.is_empty() || ip.is_empty() { return None; }
        fluid_remote::protocol::decode_handshake_key(key)?; // reject invalid keys
        Some(fluid_core::settings::RemoteDevice {
            id: new_device_id(), name: name.to_string(), host: ip.to_string(),
            port: self.settings.remote_port, key: key.to_string(),
            popout: fluid_core::settings::PopoutSettings::default(),
        })
    }
    fn popout_size(&self, po: &fluid_core::settings::PopoutSettings) -> Size {
        let tw = self.settings.tile_width;
        let th = self.settings.tile_height;
        let sp = style::skin_style(&self.settings.active_skin).tile_spacing;
        let n = [po.show_cpu, po.show_gpu, po.show_ram, po.show_network, po.show_storage]
            .iter().filter(|x| **x).count().max(1) as f32;
        // 6px popout padding each side → window = tile width + 12.
        Size::new(tw + 12.0, 12.0 + 16.0 + 4.0 + n * th + (n - 1.0) * sp + 12.0)
    }

    fn eval_warnings(&mut self) {
        self.warn_state.clear();
        for w in &self.settings.warnings {
            if !w.enabled { continue; }
            let (temp, load, used_gb): (Option<f32>, f32, f32) = match w.kind.as_str() {
                "CPU" => (self.snapshot.cpu.temperature_c, self.snapshot.cpu.usage_percent, 0.0),
                "GPU" => (self.snapshot.gpu.temperature_c, self.snapshot.gpu.usage_percent, 0.0),
                "RAM" => (None, self.snapshot.ram.usage_percent, self.snapshot.ram.used_mb / 1024.0),
                _ => continue,
            };
            let current: f64 = match w.metric {
                WarnMetric::Temperature => temp.unwrap_or(0.0) as f64,
                WarnMetric::Load => load as f64,
                WarnMetric::UsedGb => used_gb as f64,
                WarnMetric::Throughput => 0.0,
            };
            let exceeded = current >= w.threshold;
            let accent_override = if w.gradient_mode && w.metric == WarnMetric::Temperature {
                let hot = style::parse_hex(&w.gradient_color, Color::from_rgb(1.0, 0.13, 0.0));
                temp.and_then(|t| { let dist = w.threshold - t as f64; if dist <= 15.0 { Some(style::gradient_color(dist, hot)) } else { None } })
            } else { None };
            self.warn_state.insert(w.kind.clone(), (exceeded && w.flash_enabled, accent_override));
        }
    }
    fn warn_view(&self, kind: &str) -> WarnView {
        match self.warn_state.get(kind) {
            Some(&(flash, ov)) => WarnView { flash: flash && self.flash_on, accent_override: ov },
            None => WarnView::default(),
        }
    }
    fn theme_name(&self) -> String {
        style::match_preset(&self.settings).map(|i| style::THEME_PRESETS[i].0.to_string()).unwrap_or("Custom".into())
    }
    fn disk_options(&self) -> Vec<String> {
        let mut v: Vec<String> = self.snapshot.disk.drives.iter().map(|d| d.mount.trim_end_matches('\\').to_string()).collect();
        v.sort(); v.dedup(); if v.is_empty() { v.push("C:".into()); } v
    }
    fn adapter_options(&self) -> Vec<String> {
        let mut v = vec!["All adapters".to_string()];
        let mut names: Vec<String> = self.snapshot.network.interfaces.iter().map(|i| i.name.clone()).collect();
        names.sort(); names.dedup(); v.extend(names); v
    }
    // Returns the snapped position plus which edges it locked to (right/bottom),
    // so resizes can keep the snapped corner anchored.
    fn snap_with_edges(&self, pos: Point) -> Option<(Point, bool, bool)> {
        let (l, t, r, b) = work_area()?;
        let sz = self.widget_size();
        let m = self.settings.snap_distance.max(0.0);
        let mut x = pos.x; let mut y = pos.y;
        let mut sr = false; let mut sb = false;
        if (x - l).abs() < m { x = l; }
        if ((x + sz.width) - r).abs() < m { x = r - sz.width; sr = true; }
        if (y - t).abs() < m { y = t; }
        if ((y + sz.height) - b).abs() < m { y = b - sz.height; sb = true; }

        // Dock to our other windows' outer edges (e.g. the settings window).
        if self.settings.snap_to_windows {
            for (l2, t2, r2, b2) in own_window_rects(&self.settings.snap_blocklist) {
                // Only consider windows that overlap vertically/horizontally so
                // we dock side-by-side rather than to a far-away window.
                let v_overlap = y < b2 && (y + sz.height) > t2;
                let h_overlap = x < r2 && (x + sz.width) > l2;
                if v_overlap {
                    if ((x + sz.width) - l2).abs() < m { x = l2 - sz.width; }
                    else if (x - r2).abs() < m { x = r2; }
                    // align top/bottom edges with the window
                    if (y - t2).abs() < m { y = t2; }
                    else if ((y + sz.height) - b2).abs() < m { y = b2 - sz.height; }
                }
                if h_overlap {
                    if ((y + sz.height) - t2).abs() < m { y = t2 - sz.height; }
                    else if (y - b2).abs() < m { y = b2; }
                    if (x - l2).abs() < m { x = l2; }
                    else if ((x + sz.width) - r2).abs() < m { x = r2 - sz.width; }
                }
            }
        }

        if (x - pos.x).abs() > 0.5 || (y - pos.y).abs() > 0.5 || sr || sb {
            Some((Point::new(x, y), sr, sb))
        } else {
            None
        }
    }
    fn game_corner(&self) -> Option<Point> {
        let (l, t, r, b) = work_area()?;
        let sz = self.widget_size();
        const M: f32 = 8.0;
        let cx = l + ((r - l) - sz.width) / 2.0;
        let cy = t + ((b - t) - sz.height) / 2.0;
        let left = l + M;
        let right = r - sz.width - M;
        let top = t + M;
        let bottom = b - sz.height - M;
        Some(match self.settings.game_mode_position {
            SnapPosition::TopLeft => Point::new(left, top),
            SnapPosition::TopCenter => Point::new(cx, top),
            SnapPosition::TopRight => Point::new(right, top),
            SnapPosition::LeftCenter => Point::new(left, cy),
            SnapPosition::RightCenter => Point::new(right, cy),
            SnapPosition::BottomLeft => Point::new(left, bottom),
            SnapPosition::BottomCenter => Point::new(cx, bottom),
            SnapPosition::BottomRight => Point::new(right, bottom),
        })
    }

    fn snapshot_appearance(&self) -> Appearance {
        Appearance {
            bg: self.settings.theme_bg.clone(),
            tile: self.settings.theme_tile.clone(),
            accent: self.settings.theme_accent.clone(),
            text: self.settings.theme_text.clone(),
            muted: self.settings.theme_muted.clone(),
            skin: self.settings.active_skin.clone(),
            primary_font: self.settings.primary_font.clone(),
            secondary_font: self.settings.secondary_font.clone(),
            indicator_font: self.settings.indicator_font.clone(),
        }
    }
    // Save the current colors + skin into preset slot `idx` (padding empty slots
    // before it so unsaved slots stay blank and render as plain numbers).
    fn save_appearance_to_slot(&mut self, idx: usize) {
        let a = self.snapshot_appearance();
        while self.settings.presets.len() <= idx {
            self.settings.presets.push(fluid_core::settings::PresetSlot {
                name: String::new(), bg: String::new(), tile: String::new(),
                accent: String::new(), text: String::new(), muted: String::new(), skin: String::new(),
            });
        }
        let p = &mut self.settings.presets[idx];
        p.name = format!("Slot {}", idx + 1);
        p.bg = a.bg; p.tile = a.tile; p.accent = a.accent; p.text = a.text; p.muted = a.muted; p.skin = a.skin;
    }
    // Push the current appearance onto the undo stack (cap 5, like C#).
    fn push_appearance_undo(&mut self) {
        let snap = self.snapshot_appearance();
        self.appearance_undo.push(snap);
        if self.appearance_undo.len() > 5 {
            self.appearance_undo.remove(0);
        }
    }
    fn restore_appearance(&mut self, a: Appearance) {
        self.settings.theme_bg = a.bg;
        self.settings.theme_tile = a.tile;
        self.settings.theme_accent = a.accent;
        self.settings.theme_text = a.text;
        self.settings.theme_muted = a.muted;
        self.settings.active_skin = a.skin;
        self.settings.primary_font = a.primary_font;
        self.settings.secondary_font = a.secondary_font;
        self.settings.indicator_font = a.indicator_font;
    }

    // Encode the current appearance (colors + skin + fonts) to a share code.
    fn appearance_share_code(&self) -> String {
        use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
        let s = &self.settings;
        let v = serde_json::json!({
            "bg": s.theme_bg, "tile": s.theme_tile, "accent": s.theme_accent,
            "text": s.theme_text, "muted": s.theme_muted, "skin": s.active_skin,
            "pf": s.primary_font, "sf": s.secondary_font, "if": s.indicator_font,
        });
        format!("FMA1:{}", B64.encode(v.to_string()))
    }
    // Apply a share code to the current appearance. Returns false if invalid.
    fn apply_share_code(&mut self, code: &str) -> bool {
        use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
        let body = match code.trim().strip_prefix("FMA1:") { Some(b) => b, None => return false };
        let bytes = match B64.decode(body) { Ok(b) => b, Err(_) => return false };
        let text = match String::from_utf8(bytes) { Ok(t) => t, Err(_) => return false };
        let v: serde_json::Value = match serde_json::from_str(&text) { Ok(v) => v, Err(_) => return false };
        self.push_appearance_undo();
        let s = &mut self.settings;
        let str_or = |v: &serde_json::Value, k: &str, cur: &str| v[k].as_str().unwrap_or(cur).to_string();
        s.theme_bg = str_or(&v, "bg", &s.theme_bg);
        s.theme_tile = str_or(&v, "tile", &s.theme_tile);
        s.theme_accent = str_or(&v, "accent", &s.theme_accent);
        s.theme_text = str_or(&v, "text", &s.theme_text);
        s.theme_muted = str_or(&v, "muted", &s.theme_muted);
        s.active_skin = str_or(&v, "skin", &s.active_skin);
        s.primary_font = v["pf"].as_str().map(|x| x.to_string());
        s.secondary_font = v["sf"].as_str().map(|x| x.to_string());
        s.indicator_font = v["if"].as_str().map(|x| x.to_string());
        let _ = self.settings.save();
        true
    }

    // Apply (or clear) click-through on the widget window based on current mode.
    fn apply_click_through(&mut self) -> Task<Message> {
        let want = if self.game_mode {
            self.settings.game_mode_click_through
        } else {
            self.settings.click_through
        };
        if want != self.click_through_applied {
            self.click_through_applied = want;
            set_click_through(WIDGET_TITLE, want);
        }
        Task::none()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),
            Message::SensorTick => {
                let poller = self.poller.get_or_insert_with(SensorPoller::new);
                self.snapshot = poller.poll(); self.eval_warnings();
                // Feed the TCP server so connected remotes receive this machine.
                if let Some(r) = &self.remote { r.push_snapshot(self.snapshot.clone()); }
                Task::none()
            }
            Message::FlashTick => { self.flash_on = !self.flash_on; Task::none() }
            Message::AnimTick => {
                // Continuous seconds accumulator (~60ms/tick); modes derive their
                // own-speed waves from it.
                self.anim_t = (self.anim_t + 0.06) % 3600.0;
                Task::none()
            }
            Message::TrayPoll => {
                let mut tasks: Vec<Task<Message>> = Vec::new();
                if let Ok(event) = MenuEvent::receiver().try_recv() {
                    if event.id == self.exit_id { return iced::exit(); }
                    if event.id == self.settings_id { tasks.push(self.open_settings()); }
                    if event.id == self.show_id { if let Some(id) = self.widget_window() { tasks.push(window::change_mode(id, window::Mode::Windowed)); } }
                    if event.id == self.game_id {
                        tasks.push(self.toggle_game_mode());
                    }
                }
                tasks.push(self.drain_hotkey_events());
                if let Some((id, pos, when)) = self.pending_snap {
                    if when.elapsed() > Duration::from_millis(150) {
                        self.pending_snap = None;
                        match self.snap_with_edges(pos) {
                            Some((snapped, sr, sb)) => {
                                self.snap_right = sr; self.snap_bottom = sb;
                                self.ignore_next_move = true;
                                self.settings.window_x = snapped.x as f64; self.settings.window_y = snapped.y as f64;
                                let _ = self.settings.save(); tasks.push(window::move_to(id, snapped));
                            }
                            None => { self.snap_right = false; self.snap_bottom = false; }
                        }
                    }
                }
                if tasks.is_empty() { Task::none() } else { Task::batch(tasks) }
            }
            Message::DragWindow(id) => window::drag(id),
            // Snap immediately when the mouse is released after dragging.
            Message::SnapWidgetNow => {
                self.pending_snap = None;
                if self.game_mode || !self.settings.snap_to_edges { return Task::none(); }
                if let Some(id) = self.widget_window() {
                    let cur = Point::new(self.settings.window_x as f32, self.settings.window_y as f32);
                    match self.snap_with_edges(cur) {
                        Some((snapped, sr, sb)) => {
                            self.snap_right = sr; self.snap_bottom = sb;
                            self.ignore_next_move = true;
                            self.settings.window_x = snapped.x as f64;
                            self.settings.window_y = snapped.y as f64;
                            let _ = self.settings.save();
                            return window::move_to(id, snapped);
                        }
                        None => { self.snap_right = false; self.snap_bottom = false; }
                    }
                }
                Task::none()
            }
            Message::WindowOpened(id, kind) => {
                self.windows.insert(id, kind);
                if kind == WindowKind::Popout {
                    if let Some(dev) = self.pending_popout.pop_front() { self.popout_device.insert(id, dev); }
                    return self.update_widget_level();
                }
                if kind == WindowKind::Widget {
                    // Give the widget a unique OS window title (FindWindow target
                    // for snap + click-through), then apply click-through state.
                    rename_widget_window();
                    return self.apply_click_through();
                }
                // A settings/popup opened: drop the widget below it.
                self.update_widget_level()
            }
            Message::WindowMoved(id, pos) => {
                match self.windows.get(&id) {
                    Some(&WindowKind::Widget) => {
                        if self.ignore_next_move { self.ignore_next_move = false; return Task::none(); }
                        if self.game_mode { return Task::none(); }
                        self.settings.window_x = pos.x as f64; self.settings.window_y = pos.y as f64;
                        self.settings.first_run_complete = true; let _ = self.settings.save();
                        if self.settings.snap_to_edges { self.pending_snap = Some((id, pos, Instant::now())); }
                    }
                    Some(&WindowKind::Settings) => {
                        self.settings.settings_window_x = Some(pos.x as f64);
                        self.settings.settings_window_y = Some(pos.y as f64); let _ = self.settings.save();
                    }
                    Some(&k) => {
                        if let Some(key) = kind_key(k) {
                            self.settings.popup_positions.insert(key.to_string(), (pos.x as f64, pos.y as f64));
                            let _ = self.settings.save();
                        }
                    }
                    None => {}
                }
                Task::none()
            }
            Message::WindowClosed(id) => { self.windows.remove(&id); self.popout_device.remove(&id); if self.widget_window().is_none() { return iced::exit(); } self.update_widget_level() }
            Message::OpenSettings => self.open_settings(),
            Message::HideWidget => self.widget_window().map(|id| window::change_mode(id, window::Mode::Hidden)).unwrap_or(Task::none()),
            Message::OpenAlerts => self.open_popup(WindowKind::Alerts, popups::ALERTS_SIZE),
            Message::OpenGameMode => self.open_popup(WindowKind::GameMode, popups::GAME_MODE_SIZE),
            Message::OpenHelp => self.open_popup(WindowKind::Help, popups::HELP_SIZE),
            Message::OpenUtilities => self.open_popup(WindowKind::Utilities, popups::UTILITIES_SIZE),
            // ── Optional CPU sensor driver (PawnIO) ──
            Message::OpenCpuDriver => {
                self.cpu_driver_installed = cpu_driver::is_installed();
                self.cpu_dialog = CpuDriverStage::Primary;
                self.open_popup(WindowKind::CpuDriver, popups::CPU_DRIVER_SIZE)
            }
            Message::DismissCpuTempHint => {
                self.settings.cpu_temp_hint_dismissed = true;
                let _ = self.settings.save();
                Task::none()
            }
            Message::SwitchWidgetDevice(dev) => {
                // Ignore unknown ids; None always valid (this PC).
                self.widget_device = dev.filter(|id| self.settings.remote_devices.iter().any(|d| &d.id == id));
                Task::none()
            }
            Message::SetShowRemoteStatusDot(on) => {
                self.settings.show_remote_status_dot = on;
                let _ = self.settings.save();
                Task::none()
            }
            Message::ToggleTileSection(name) => {
                self.tiles_section = if self.tiles_section.as_deref() == Some(name.as_str()) { None } else { Some(name) };
                Task::none()
            }
            Message::SetTileField(key, on) => {
                let st = &mut self.settings;
                match key.as_str() {
                    "cpu_temp" => st.cpu_show_temp = on,
                    "cpu_clock" => st.cpu_show_clock = on,
                    "gpu_temp" => st.gpu_show_temp = on,
                    "gpu_clock" => st.gpu_show_clock = on,
                    "gpu_vram" => st.gpu_show_vram = on,
                    "ram_speed" => st.ram_show_speed = on,
                    "ram_details" => st.ram_show_details = on,
                    "net_down" => st.net_show_down = on,
                    "net_up" => st.net_show_up = on,
                    "disk_read" => st.disk_show_read = on,
                    "disk_write" => st.disk_show_write = on,
                    "clock_date" => st.clock_show_date = on,
                    _ => {}
                }
                let _ = self.settings.save();
                Task::none()
            }
            Message::CpuDriverMoreInfo => { self.cpu_dialog = CpuDriverStage::Info; Task::none() }
            Message::CpuDriverBack => { self.cpu_dialog = CpuDriverStage::Primary; Task::none() }
            Message::CpuDriverInstall => {
                self.cpu_dialog = CpuDriverStage::Progress("Downloading and verifying the sensor driver…".into());
                Task::perform(cpu_driver::install(), Message::CpuDriverInstallDone)
            }
            Message::CpuDriverUninstall => {
                self.cpu_dialog = CpuDriverStage::Progress("Removing the sensor driver…".into());
                Task::perform(cpu_driver::uninstall(), Message::CpuDriverUninstallDone)
            }
            Message::CpuDriverInstallDone(outcome) => {
                self.cpu_driver_installed = cpu_driver::is_installed();
                // Re-probe sensors so a just-installed driver lights up the CPU
                // temperature without an app restart (C# RecheckSensors parity).
                fluid_sensor::refresh_cpu_temp_driver();
                self.cpu_dialog = match outcome.result {
                    cpu_driver::InstallResult::Installed | cpu_driver::InstallResult::AlreadyPresent =>
                        CpuDriverStage::Done {
                            ok: true,
                            title: "CPU temperature is on".into(),
                            body: "The sensor driver is installed. Your CPU temperature now appears on the widget.".into(),
                            show_fallback: false,
                        },
                    // User declined the UAC prompt — back out, no error.
                    cpu_driver::InstallResult::Cancelled => CpuDriverStage::Primary,
                    cpu_driver::InstallResult::Failed => CpuDriverStage::Done {
                        ok: false,
                        title: "Automatic setup didn't finish".into(),
                        body: if outcome.detail.is_empty() { "The automatic install didn't complete.".into() } else { outcome.detail },
                        show_fallback: true,
                    },
                };
                Task::none()
            }
            Message::CpuDriverUninstallDone(outcome) => {
                self.cpu_driver_installed = cpu_driver::is_installed();
                fluid_sensor::refresh_cpu_temp_driver();
                self.cpu_dialog = match outcome.result {
                    cpu_driver::InstallResult::Installed | cpu_driver::InstallResult::AlreadyPresent =>
                        CpuDriverStage::Done {
                            ok: true,
                            title: "Sensor driver removed".into(),
                            body: "The CPU sensor driver was removed. CPU temperature returns to the opt-in hint.".into(),
                            show_fallback: false,
                        },
                    cpu_driver::InstallResult::Cancelled => CpuDriverStage::Primary,
                    cpu_driver::InstallResult::Failed => CpuDriverStage::Done {
                        ok: false,
                        title: "Couldn't remove the driver".into(),
                        body: if outcome.detail.is_empty() { "The uninstaller didn't complete.".into() } else { outcome.detail },
                        show_fallback: false,
                    },
                };
                Task::none()
            }
            Message::OpenUrl(url) => { open_url(&url); Task::none() }
            Message::OpenThemeStore => { self.theme_store_franchise = None; self.open_popup(WindowKind::ThemeStore, popups::THEME_STORE_SIZE) }
            Message::ThemeStoreOpenFranchise(i) => { self.theme_store_franchise = Some(i); Task::none() }
            Message::ThemeStoreBack => { self.theme_store_franchise = None; Task::none() }
            Message::ApplyPackTheme(pack_idx, theme_idx) => {
                if let Some(pack) = style::theme_packs().get(pack_idx) {
                    if let Some(theme) = pack.themes.get(theme_idx) {
                        self.push_appearance_undo();
                        style::apply_pack_theme(&mut self.settings, theme);
                        let _ = self.settings.save();
                        return self.resize_widget();
                    }
                }
                Task::none()
            }
            Message::BlocklistAction(action) => { self.blocklist_editor.perform(action); Task::none() }
            Message::SaveBlocklist => {
                let lines: Vec<String> = self.blocklist_editor.text()
                    .lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
                let n = lines.len();
                self.settings.snap_blocklist = lines;
                let _ = self.settings.save();
                self.blocklist_status = format!("Saved ({} rule{})", n, if n == 1 { "" } else { "s" });
                Task::none()
            }
            Message::PickWindow => self.open_popup(WindowKind::WindowPicker, popups::WINDOW_PICKER_SIZE),
            Message::PickWindowChosen(title) => {
                let text = self.blocklist_editor.text().trim_end().to_string();
                let combined = if text.is_empty() { title } else { format!("{text}\n{title}") };
                self.blocklist_editor = iced::widget::text_editor::Content::with_text(&combined);
                self.blocklist_status = "Window added (click Save)".into();
                self.close_kind(WindowKind::WindowPicker)
            }
            Message::ClosePopup(id) => {
                let _ = self.settings.save();
                Task::batch([window::close(id), self.resize_widget()])
            }
            // C# widget Window.ContextMenu (right-click): Settings… / Exit.
            Message::ShowWidgetMenu => {
                if self.windows.values().any(|k| *k == WindowKind::WidgetMenu) { return Task::none(); }
                // Anchor the menu at the cursor (like the C# ContextMenu),
                // falling back to the widget top-left if unavailable.
                let pos = match cursor_logical_pos() {
                    Some((x, y)) => Point::new(x, y),
                    None => Point::new(self.settings.window_x as f32 + 8.0, self.settings.window_y as f32 + 26.0),
                };
                let (_, t) = window::open(window::Settings {
                    size: popups::WIDGET_MENU_SIZE, position: window::Position::Specific(pos),
                    decorations: false, transparent: true, resizable: false,
                    level: window::Level::AlwaysOnTop, platform_specific: no_taskbar(), ..Default::default()
                });
                t.map(|id| Message::WindowOpened(id, WindowKind::WidgetMenu))
            }
            Message::WidgetMenuSettings => Task::batch([self.close_kind(WindowKind::WidgetMenu), self.open_settings()]),
            Message::WidgetMenuExit => iced::exit(),
            Message::WindowUnfocused(id) => {
                // Dismiss the context menu when it loses focus (click elsewhere).
                if self.windows.get(&id) == Some(&WindowKind::WidgetMenu) {
                    return window::close(id);
                }
                Task::none()
            }
            Message::SaveClose => {
                let _ = self.settings.save();
                let close = self.settings_window().map(window::close).unwrap_or(Task::none());
                Task::batch([close, self.resize_widget()])
            }
            Message::ResetDefaults => {
                let keep = (self.settings.window_x, self.settings.window_y, self.settings.first_run_complete);
                self.settings = AppSettings::default();
                self.settings.window_x = keep.0; self.settings.window_y = keep.1; self.settings.first_run_complete = keep.2;
                self.resize_widget()
            }
            Message::ToggleTile(name, on) => {
                if on { if !self.settings.visible_tiles.contains(&name) { self.settings.visible_tiles.push(name.clone()); }
                    if !self.settings.tile_order.contains(&name) { self.settings.tile_order.push(name); }
                } else { self.settings.visible_tiles.retain(|t| t != &name); }
                self.resize_widget()
            }
            Message::SetOpacity(v) => { self.settings.widget_opacity = v; Task::none() }
            Message::SetOrientation(o) => { self.settings.orientation = o; self.resize_widget() }
            Message::SetFahrenheit(f) => { self.settings.temperature_unit = if f { TempUnit::Fahrenheit } else { TempUnit::Celsius }; Task::none() }
            Message::SetSnap(on) => {
                self.settings.snap_to_edges = on;
                // Enabling edge-snap turns window-snap on by default (it's a
                // sub-option that only appears while edge-snap is on).
                if on { self.settings.snap_to_windows = true; }
                else { self.snap_right = false; self.snap_bottom = false; }
                Task::none()
            }
            // (theme accent edited via the colour swatches / hex editor)
            Message::ThemePrev => {
                self.push_appearance_undo();
                let n = style::THEME_PRESETS.len();
                let idx = style::match_preset(&self.settings).map(|i| (i + n - 1) % n).unwrap_or(n - 1);
                style::apply_preset(&mut self.settings, idx);
                let _ = self.settings.save(); Task::none()
            }
            Message::ThemeNext => {
                self.push_appearance_undo();
                let n = style::THEME_PRESETS.len();
                let idx = style::match_preset(&self.settings).map(|i| (i + 1) % n).unwrap_or(0);
                style::apply_preset(&mut self.settings, idx);
                let _ = self.settings.save(); Task::none()
            }
            // Moon / sun toggles: apply the Dark / Light default palette (presets
            // 0 and 1 in THEME_PRESETS).
            Message::SetColorMode(dark) => {
                self.push_appearance_undo();
                style::apply_preset(&mut self.settings, if dark { 0 } else { 1 });
                let _ = self.settings.save();
                Task::none()
            }
            Message::SetWarnEnabled(k, on) => { self.settings.warn_mut(&k).enabled = on; self.eval_warnings(); Task::none() }
            Message::SetWarnThresholdStr(k, s) => {
                let v: f64 = s.trim().parse().unwrap_or(0.0);
                self.settings.warn_mut(&k).threshold = v.clamp(0.0, 1000.0); self.eval_warnings(); Task::none()
            }
            Message::SetWarnFlash(k, on) => { self.settings.warn_mut(&k).flash_enabled = on; self.eval_warnings(); Task::none() }
            Message::SetWarnGradient(k, on) => { self.settings.warn_mut(&k).gradient_mode = on; self.eval_warnings(); Task::none() }
            Message::SetWarnMetric(k, m) => { self.settings.warn_mut(&k).metric = m; self.eval_warnings(); Task::none() }
            Message::SetWarnFlashColor(k, s) => { self.settings.warn_mut(&k).flash_color = s; let _ = self.settings.save(); Task::none() }
            Message::SetWarnGradientColor(k, s) => { self.settings.warn_mut(&k).gradient_color = s; let _ = self.settings.save(); Task::none() }
            Message::EditColor(slot) => {
                self.editing_color = if self.editing_color == Some(slot) { None } else { Some(slot) };
                Task::none()
            }
            Message::SetHexColor(slot, v) => {
                match slot { 0 => self.settings.theme_bg = v, 1 => self.settings.theme_tile = v, 2 => self.settings.theme_accent = v, 3 => self.settings.theme_text = v, _ => self.settings.theme_muted = v }
                let _ = self.settings.save();
                Task::none()
            }
            Message::SetTileWidth(v) => { self.settings.tile_width = v; self.resize_widget() }
            Message::SetTileHeight(v) => { self.settings.tile_height = v; self.resize_widget() }
            Message::SetPrimaryFontOffset(v) => { self.settings.primary_font_offset = v as i32; Task::none() }
            Message::SetSecondaryFontOffset(v) => { self.settings.secondary_font_offset = v as i32; Task::none() }
            Message::SetIndicatorFontOffset(v) => { self.settings.indicator_font_offset = v as i32; Task::none() }
            Message::SetMutedContrast(v) => { self.settings.muted_contrast = v; Task::none() }
            Message::SetInterval(v) => { self.settings.update_interval_ms = v as u64; Task::none() }
            Message::SetCpuName(v) => { self.settings.cpu_custom_name = v; Task::none() }
            Message::SetGpuName(v) => { self.settings.gpu_custom_name = v; Task::none() }
            Message::SetDisk(v) => { self.settings.selected_disk_mount = v; Task::none() }
            Message::SetAdapter(v) => { self.settings.network_adapter_name = if v == "All adapters" { String::new() } else { v }; Task::none() }
            Message::SetAlwaysOnTop(on) => {
                self.settings.always_on_top = on;
                self.update_widget_level()
            }
            Message::SetRunAtStartup(on) => { self.settings.run_at_startup = on; set_run_at_startup(on); Task::none() }
            Message::SetUiScale(v) => { self.settings.ui_scale = v; self.resize_widget() }
            Message::SetClickThrough(on) => { self.settings.click_through = on; self.apply_click_through() }
            Message::SetSnapWindows(on) => { self.settings.snap_to_windows = on; Task::none() }
            Message::SetSnapDistance(v) => { self.settings.snap_distance = v; Task::none() }
            Message::TrafficCycle => {
                let modes = ["Off", "Blink", "Fade", "Glow"];
                let cur = modes.iter().position(|m| *m == self.settings.network_traffic_indicator).unwrap_or(0);
                self.settings.network_traffic_indicator = modes[(cur + 1) % modes.len()].to_string();
                Task::none()
            }
            Message::SetArrowSpacing(v) => { self.settings.network_arrow_spacing = v; Task::none() }
            Message::SetArrowFontOffset(v) => { self.settings.arrow_font_offset = v as i32; Task::none() }
            Message::SetDiskLabelSpacing(v) => { self.settings.disk_label_spacing = v; Task::none() }
            Message::SetDiskLabelFontOffset(v) => { self.settings.disk_label_font_offset = v as i32; Task::none() }
            Message::SetSettingsTab(i) => {
                self.settings_tab = i;
                self.settings_window().map(|id| window::resize(id, settings_size_for_tab(i))).unwrap_or(Task::none())
            }
            Message::ArmHotkey(target) => {
                // Toggle capture: clicking the armed field again disarms it.
                self.capturing_hotkey = if self.capturing_hotkey == Some(target) { None } else { Some(target) };
                Task::none()
            }
            Message::HotkeyKeyPressed(key, mods) => {
                let target = match self.capturing_hotkey { Some(t) => t, None => return Task::none() };
                // Escape cancels capture without binding (matches C#).
                if matches!(key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)) {
                    self.capturing_hotkey = None;
                    return Task::none();
                }
                if let Some(combo) = hotkeys::format_combo(&key, mods) {
                    self.set_hotkey(target, combo);
                    self.capturing_hotkey = None;
                }
                Task::none()
            }
            Message::ClearHotkey(target) => {
                self.set_hotkey(target, String::new());
                self.capturing_hotkey = None;
                Task::none()
            }
            Message::RemotePoll => { self.drain_remote_events(); Task::none() }
            Message::ToggleRemoteSection(on) => { self.remote_expanded = on; Task::none() }
            Message::SetTcpFeedEnabled(on) => {
                self.settings.remote_enabled = on;
                // Add the firewall rule the first time the feed is enabled (one
                // UAC), so Windows won't pop the raw "allow app" dialog on bind.
                if on && !self.settings.remote_firewall_configured {
                    firewall::ensure_rule(self.settings.remote_port);
                    self.settings.remote_firewall_configured = true;
                }
                if let Some(r) = &self.remote { r.set_server_enabled(on); }
                let _ = self.settings.save();
                Task::none()
            }
            Message::CopyHandshakeKey => {
                let key = self.settings.remote_key.clone();
                if !key.is_empty() { return iced::clipboard::write(key); }
                Task::none()
            }
            Message::RegenerateKey => {
                if let Some(r) = &self.remote { r.regenerate_key(); }
                Task::none()
            }
            Message::ShowAddDevice => {
                self.add_device_open = true;
                self.new_device_name.clear(); self.new_device_ip.clear(); self.new_device_key.clear();
                self.device_test_status.clear();
                Task::none()
            }
            Message::CancelAddDevice => { self.add_device_open = false; Task::none() }
            Message::SetNewDeviceName(v) => { self.new_device_name = v; Task::none() }
            Message::SetNewDeviceIp(v) => { self.new_device_ip = v; Task::none() }
            Message::SetNewDeviceKey(v) => { self.new_device_key = v; Task::none() }
            Message::TestDevice => {
                match self.build_device_from_form() {
                    Some(dev) => {
                        self.device_test_status = "Testing\u{2026}".into();
                        if let Some(r) = &self.remote { r.test_device(dev.host, dev.port, dev.key); }
                    }
                    None => { self.device_test_status = "Fill in all fields first".into(); self.device_test_ok = false; }
                }
                Task::none()
            }
            Message::SaveDevice => {
                if self.settings.remote_devices.len() >= 5 { return Task::none(); }
                match self.build_device_from_form() {
                    Some(dev) => {
                        self.settings.remote_devices.push(dev);
                        let _ = self.settings.save();
                        if let Some(r) = &self.remote { r.set_devices(self.settings.remote_devices.clone()); }
                        self.add_device_open = false;
                    }
                    None => { self.device_test_status = "Fill in all fields first".into(); self.device_test_ok = false; }
                }
                Task::none()
            }
            Message::RemoveDevice(id) => {
                self.settings.remote_devices.retain(|d| d.id != id);
                self.remote_snapshots.remove(&id); self.remote_conn.remove(&id);
                if self.widget_device.as_deref() == Some(id.as_str()) { self.widget_device = None; }
                let _ = self.settings.save();
                if let Some(r) = &self.remote { r.set_devices(self.settings.remote_devices.clone()); }
                Task::none()
            }
            Message::OpenPopout(id) => {
                let dev = self.settings.remote_devices.iter().find(|d| d.id == id).cloned();
                match dev {
                    Some(dev) => {
                        self.pending_popout.push_back(dev.id.clone());
                        let size = self.popout_size(&dev.popout);
                        let (_, t) = window::open(window::Settings {
                            size, position: self.popup_position(WindowKind::Popout), decorations: false, transparent: true,
                            resizable: false, level: window::Level::AlwaysOnTop, platform_specific: no_taskbar(), ..Default::default()
                        });
                        t.map(|wid| Message::WindowOpened(wid, WindowKind::Popout))
                    }
                    None => Task::none(),
                }
            }
            Message::OpenPopoutConfig(id) => {
                self.config_device = Some(id);
                self.open_popup(WindowKind::PopoutConfig, popups::POPOUT_CONFIG_SIZE)
            }
            Message::PopoutSyncColors(id, on) => {
                let main = (self.settings.theme_bg.clone(), self.settings.theme_tile.clone(),
                    self.settings.theme_accent.clone(), self.settings.theme_text.clone(), self.settings.theme_muted.clone());
                if let Some(d) = self.device_mut(&id) {
                    d.popout.sync_colors = on;
                    // Seed device colours from the current theme when first unsynced.
                    if !on && d.popout.bg.is_empty() {
                        d.popout.bg = main.0; d.popout.tile = main.1; d.popout.accent = main.2;
                        d.popout.text = main.3; d.popout.muted = main.4;
                    }
                }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutColor(id, slot, hex) => {
                if let Some(d) = self.device_mut(&id) {
                    match slot { 0 => d.popout.bg = hex, 1 => d.popout.tile = hex, 2 => d.popout.accent = hex,
                        3 => d.popout.text = hex, _ => d.popout.muted = hex }
                }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutOpacity(id, v) => {
                if let Some(d) = self.device_mut(&id) { d.popout.opacity = v; }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutTile(id, tile, on) => {
                if let Some(d) = self.device_mut(&id) {
                    match tile.as_str() {
                        "CPU" => d.popout.show_cpu = on, "GPU" => d.popout.show_gpu = on,
                        "RAM" => d.popout.show_ram = on, "Network" => d.popout.show_network = on,
                        "Storage" => d.popout.show_storage = on, _ => {}
                    }
                }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutLabel(id, which, v) => {
                if let Some(d) = self.device_mut(&id) {
                    if which == 0 { d.popout.cpu_label = v; } else { d.popout.gpu_label = v; }
                }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutWarnEnabled(id, kind, on) => {
                if let Some(d) = self.device_mut(&id) { d.popout.warn_mut(&kind).enabled = on; }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutWarnMetric(id, kind, m) => {
                if let Some(d) = self.device_mut(&id) { d.popout.warn_mut(&kind).metric = m; }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutWarnThreshold(id, kind, s) => {
                let parsed = s.trim().parse::<f64>().ok();
                if let Some(d) = self.device_mut(&id) {
                    if let Some(v) = parsed { d.popout.warn_mut(&kind).threshold = v; }
                    else if s.trim().is_empty() { d.popout.warn_mut(&kind).threshold = 0.0; }
                }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutWarnFlash(id, kind, on) => {
                if let Some(d) = self.device_mut(&id) { d.popout.warn_mut(&kind).flash_enabled = on; }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutWarnFlashColor(id, kind, s) => {
                if let Some(d) = self.device_mut(&id) { d.popout.warn_mut(&kind).flash_color = s; }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutWarnGradient(id, kind, on) => {
                if let Some(d) = self.device_mut(&id) { d.popout.warn_mut(&kind).gradient_mode = on; }
                let _ = self.settings.save();
                Task::none()
            }
            Message::PopoutWarnGradientColor(id, kind, s) => {
                if let Some(d) = self.device_mut(&id) { d.popout.warn_mut(&kind).gradient_color = s; }
                let _ = self.settings.save();
                Task::none()
            }
            Message::SkinPrev => {
                self.push_appearance_undo();
                let skins = style::skin_names();
                let cur = skins.iter().position(|s| *s == self.settings.active_skin).unwrap_or(0);
                self.settings.active_skin = skins[(cur + skins.len() - 1) % skins.len()].to_string();
                let _ = self.settings.save();
                self.resize_widget()
            }
            Message::SkinNext => {
                self.push_appearance_undo();
                let skins = style::skin_names();
                let cur = skins.iter().position(|s| *s == self.settings.active_skin).unwrap_or(0);
                self.settings.active_skin = skins[(cur + 1) % skins.len()].to_string();
                let _ = self.settings.save();
                self.resize_widget()
            }
            // C# OnRandomizeAppearance (left-click skin dice): random skin AND a
            // random colour palette (a mashup); fonts too if RandomizeFontsOnDice.
            Message::RandomizeAppearance => {
                self.push_appearance_undo();
                let r = nanos();
                let skins = style::skin_names();
                let mut si = r % skins.len();
                if skins[si] == self.settings.active_skin { si = (si + 1) % skins.len(); }
                self.settings.active_skin = skins[si].to_string();

                let n = style::THEME_PRESETS.len();
                let ti = (r / 7 + 1) % n;
                style::apply_preset(&mut self.settings, ti);

                if self.settings.randomize_fonts_on_dice && !self.font_list.is_empty() {
                    let fl = &self.font_list;
                    let primary = fl[(r / 13) % fl.len()].clone();
                    if self.settings.sync_fonts {
                        self.settings.primary_font = Some(primary.clone());
                        self.settings.secondary_font = Some(primary.clone());
                        self.settings.indicator_font = Some(primary);
                    } else {
                        self.settings.primary_font = Some(primary);
                        self.settings.secondary_font = Some(fl[(r / 17) % fl.len()].clone());
                        self.settings.indicator_font = Some(fl[(r / 19) % fl.len()].clone());
                    }
                }
                let _ = self.settings.save();
                self.resize_widget()
            }
            // C# OnRandomizeSkinOnly (right-click skin dice): random skin, keep
            // colours and fonts untouched.
            Message::RandomizeSkinOnly => {
                self.push_appearance_undo();
                let skins = style::skin_names();
                let mut idx = nanos() % skins.len();
                if skins[idx] == self.settings.active_skin { idx = (idx + 1) % skins.len(); }
                self.settings.active_skin = skins[idx].to_string();
                let _ = self.settings.save();
                self.resize_widget()
            }
            // C# OnUndoAppearance: revert the last appearance change (up to 5).
            Message::UndoAppearance => {
                if let Some(prev) = self.appearance_undo.pop() {
                    self.restore_appearance(prev);
                }
                let _ = self.settings.save();
                self.resize_widget()
            }
            Message::SetSyncFonts(on) => { self.settings.sync_fonts = on; let _ = self.settings.save(); Task::none() }
            Message::SetRandomizeFonts(on) => { self.settings.randomize_fonts_on_dice = on; let _ = self.settings.save(); Task::none() }
            Message::SetFont(slot, name) => {
                let val = if name.is_empty() { None } else { Some(name) };
                if self.settings.sync_fonts {
                    self.settings.primary_font = val.clone();
                    self.settings.secondary_font = val.clone();
                    self.settings.indicator_font = val;
                } else {
                    match slot {
                        0 => self.settings.primary_font = val,
                        1 => self.settings.secondary_font = val,
                        _ => self.settings.indicator_font = val,
                    }
                }
                let _ = self.settings.save();
                Task::none()
            }
            Message::SetUpdateMode(mode) => {
                self.settings.update_check_mode = match mode.as_str() {
                    "Auto" => fluid_core::settings::UpdateMode::Auto,
                    "Off" => fluid_core::settings::UpdateMode::Off,
                    _ => fluid_core::settings::UpdateMode::Manual,
                };
                let _ = self.settings.save();
                Task::none()
            }
            Message::CheckForUpdates => {
                if self.update_checking { return Task::none(); }
                self.update_checking = true;
                self.update_status = "Checking\u{2026}".into();
                self.update_status_kind = 0;
                self.update_available = None;
                Task::perform(updates::check(env!("CARGO_PKG_VERSION").to_string()), Message::UpdateCheckDone)
            }
            Message::UpdateCheckDone(result) => {
                self.update_checking = false;
                self.settings.last_update_check = Some(chrono::Local::now().to_rfc3339());
                let _ = self.settings.save();
                match result {
                    updates::CheckResult::UpToDate => { self.update_status = "Up to date".into(); self.update_status_kind = 1; }
                    updates::CheckResult::Available(mut update) => {
                        self.update_status = String::new();
                        update.changelog = updates::changelog_bullets(&update.changelog);
                        self.update_available = Some(update);
                    }
                    updates::CheckResult::Failed(e) => {
                        tracing::debug!("update check failed: {e}");
                        self.update_status = "Check failed \u{2014} try again later".into();
                        self.update_status_kind = 2;
                    }
                }
                Task::none()
            }
            Message::DownloadUpdate => {
                let (url, sha) = match &self.update_available {
                    Some(u) => (u.url.clone(), u.sha256.clone()),
                    None => return Task::none(),
                };
                self.update_status = "Downloading\u{2026}".into();
                self.update_status_kind = 0;
                Task::perform(updates::download_and_launch(url, sha), Message::UpdateDownloadDone)
            }
            Message::UpdateDownloadDone(result) => match result {
                Ok(()) => iced::exit(),
                Err(e) => { self.update_status = format!("Download failed: {e}"); self.update_status_kind = 2; Task::none() }
            },
            Message::UpdateLater => { self.update_available = None; self.update_status = String::new(); Task::none() }
            Message::ExportAppearance => {
                self.appearance_status = "Copied to clipboard".into();
                iced::clipboard::write(self.appearance_share_code())
            }
            Message::ImportAppearance => iced::clipboard::read().map(Message::ImportAppearanceCode),
            Message::ImportAppearanceCode(opt) => {
                match opt {
                    Some(code) if self.apply_share_code(&code) => self.appearance_status = "Imported".into(),
                    _ => self.appearance_status = "No valid code on clipboard".into(),
                }
                Task::none()
            }
            Message::PresetSlotClick(slot) => {
                let idx = slot as usize;
                let saved = self.settings.presets.get(idx).is_some_and(|p| !p.accent.is_empty());
                if self.preset_arming == Some(slot) {
                    // Second click on an armed slot → save the current theme there.
                    self.save_appearance_to_slot(idx);
                    self.preset_arming = None;
                    self.appearance_status = "Saved".into();
                    let _ = self.settings.save();
                    Task::none()
                } else if saved {
                    // Click a saved slot → apply it.
                    self.preset_arming = None;
                    self.push_appearance_undo();
                    let p = self.settings.presets[idx].clone();
                    self.settings.theme_bg = p.bg;
                    self.settings.theme_tile = p.tile;
                    self.settings.theme_accent = p.accent;
                    self.settings.theme_text = p.text;
                    self.settings.theme_muted = p.muted;
                    self.settings.active_skin = p.skin;
                    let _ = self.settings.save();
                    self.resize_widget()
                } else {
                    // First click on an empty slot → arm it (shows the save icon).
                    self.preset_arming = Some(slot);
                    Task::none()
                }
            }
            // Right-click a saved slot → ask to delete it.
            Message::ConfirmDeletePreset(slot) => {
                let idx = slot as usize;
                if self.settings.presets.get(idx).is_some_and(|p| !p.accent.is_empty()) {
                    self.confirm_delete_slot = Some(slot);
                    self.open_popup(WindowKind::ConfirmDelete, popups::CONFIRM_DELETE_SIZE)
                } else {
                    Task::none()
                }
            }
            Message::DeletePresetConfirmed => {
                if let Some(slot) = self.confirm_delete_slot.take() {
                    let idx = slot as usize;
                    if idx < self.settings.presets.len() {
                        self.settings.presets[idx] = fluid_core::settings::PresetSlot {
                            name: String::new(), bg: String::new(), tile: String::new(),
                            accent: String::new(), text: String::new(), muted: String::new(), skin: String::new(),
                        };
                        while self.settings.presets.last().is_some_and(|p| p.accent.is_empty()) {
                            self.settings.presets.pop();
                        }
                        let _ = self.settings.save();
                    }
                }
                self.close_kind(WindowKind::ConfirmDelete)
            }
            Message::OpenThemePicker => { self.picker_skins = false; self.open_popup(WindowKind::Picker, popups::PICKER_SIZE) }
            Message::OpenSkinPicker => { self.picker_skins = true; self.open_popup(WindowKind::Picker, popups::PICKER_SIZE) }
            Message::ApplyThemePreset(i) => {
                self.push_appearance_undo();
                style::apply_preset(&mut self.settings, i);
                let _ = self.settings.save();
                Task::batch([self.close_kind(WindowKind::Picker), self.resize_widget()])
            }
            Message::ApplySkin(name) => {
                self.push_appearance_undo();
                self.settings.active_skin = name;
                let _ = self.settings.save();
                Task::batch([self.close_kind(WindowKind::Picker), self.resize_widget()])
            }
            Message::DiskLabelCycle => {
                // C# cycle: Drive letter > Model > Both.
                let styles = ["Letter", "Model", "Both"];
                let cur = styles.iter().position(|s| *s == self.settings.disk_label_style).unwrap_or(0);
                self.settings.disk_label_style = styles[(cur + 1) % styles.len()].to_string();
                Task::none()
            }
            Message::SetGameModeEnabled(on) => { self.settings.game_mode_enabled = on; Task::none() }
            Message::SetGameModePosition(pos) => {
                self.settings.game_mode_position = pos;
                if self.game_mode {
                    if let (Some(id), Some(c)) = (self.widget_window(), self.game_corner()) {
                        self.ignore_next_move = true;
                        return window::move_to(id, c);
                    }
                }
                Task::none()
            }
            Message::SetGameModeOpacity(v) => { self.settings.game_mode_opacity = v; Task::none() }
            Message::SetGameModeOrientation(s) => {
                self.settings.game_mode_orientation = s;
                if self.game_mode { self.resize_widget() } else { Task::none() }
            }
            Message::SetGameModeClickThrough(on) => { self.settings.game_mode_click_through = on; self.apply_click_through() }
            Message::ToggleGameModeTile(name, on) => {
                if on {
                    if !self.settings.game_mode_tiles.contains(&name) { self.settings.game_mode_tiles.push(name); }
                } else {
                    self.settings.game_mode_tiles.retain(|t| t != &name);
                }
                if self.game_mode { self.resize_widget() } else { Task::none() }
            }
        }
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        let kind = self.windows.get(&id).copied().unwrap_or(WindowKind::Widget);
        let opacity = if kind == WindowKind::Widget {
            if self.game_mode { self.settings.game_mode_opacity } else { self.settings.widget_opacity }
        } else {
            1.0
        };
        let p = Palette::from_settings(&self.settings, opacity);
        match kind {
            WindowKind::Settings => {
                let cpu_name = fmt::shorten(&self.snapshot.cpu.name);
                let gpu_name = fmt::shorten(&self.snapshot.gpu.name);
                let remote = settings_panel::RemoteView {
                    expanded: self.remote_expanded,
                    feed_on: self.settings.remote_enabled,
                    handshake_key: self.settings.remote_key.clone(),
                    devices: self.settings.remote_devices.clone(),
                    conn: self.remote_conn.clone(),
                    add_open: self.add_device_open,
                    new_name: self.new_device_name.clone(),
                    new_ip: self.new_device_ip.clone(),
                    new_key: self.new_device_key.clone(),
                    test_status: self.device_test_status.clone(),
                    test_ok: self.device_test_ok,
                };
                let capturing_ct = self.capturing_hotkey == Some(hotkeys::HotkeyTarget::ClickThrough);
                let update = settings_panel::UpdateView {
                    current_version: env!("CARGO_PKG_VERSION").to_string(),
                    mode: self.settings.update_check_mode.clone(),
                    last_checked: self.last_checked_label(),
                    status: self.update_status.clone(),
                    status_kind: self.update_status_kind,
                    available: self.update_available.as_ref().map(|u| (u.version.clone(), u.changelog.clone())),
                };
                settings_panel::view(&self.settings, p, id, self.theme_name(), self.disk_options(), self.adapter_options(), self.font_list.clone(), cpu_name, gpu_name, self.editing_color, self.settings_tab, capturing_ct, self.appearance_status.clone(), remote, update, self.cpu_driver_installed, self.tiles_section.clone(), self.preset_arming)
            }
            WindowKind::Alerts => popups::alerts_view(&self.settings, p, id),
            WindowKind::GameMode => popups::game_mode_view(&self.settings, p, id, self.capturing_hotkey == Some(hotkeys::HotkeyTarget::GameMode)),
            WindowKind::Help => popups::help_view(&self.settings, p, id),
            WindowKind::Utilities => popups::utilities_view(&self.blocklist_editor, &self.blocklist_status, p, id),
            WindowKind::WindowPicker => popups::window_picker_view(enum_window_titles(), p, id),
            WindowKind::ThemeStore => popups::theme_store_view(self.theme_store_franchise, p, id),
            WindowKind::PopoutConfig => {
                let dev = self.config_device.as_ref()
                    .and_then(|cid| self.settings.remote_devices.iter().find(|d| &d.id == cid));
                popups::popout_config_view(dev, p, id)
            }
            WindowKind::CpuDriver => popups::cpu_driver_view(&self.cpu_dialog, self.cpu_driver_installed, p, id),
            WindowKind::Picker => popups::picker_view(self.picker_skins, &self.settings, p, id),
            WindowKind::ConfirmDelete => popups::confirm_delete_view(self.confirm_delete_slot, p, id),
            WindowKind::WidgetMenu => popups::widget_menu_view(p),
            WindowKind::Popout => self.popout_view(id, p),
            WindowKind::Widget => self.widget_view(id, p),
        }
    }

    fn popout_view(&self, id: window::Id, _p: Palette) -> Element<'_, Message> {
        let dev_id = self.popout_device.get(&id).cloned().unwrap_or_default();
        let dev = self.settings.remote_devices.iter().find(|d| d.id == dev_id);
        let name = dev.map(|d| d.name.clone()).unwrap_or_else(|| "Remote".into());
        let connected = self.remote_conn.get(&dev_id).copied().unwrap_or(false);
        let snap = self.remote_snapshots.get(&dev_id).cloned().unwrap_or_default();
        let no_warn = tile::WarnView { flash: false, accent_override: None };

        // Build a per-device settings view: its colours (unless synced), opacity,
        // labels, and tile subset. Tile fns don't borrow `s` into their output.
        let po = dev.map(|d| d.popout.clone()).unwrap_or_default();
        let mut s = self.settings.clone();
        if !po.sync_colors {
            s.theme_bg = po.bg.clone(); s.theme_tile = po.tile.clone(); s.theme_accent = po.accent.clone();
            s.theme_text = po.text.clone(); s.theme_muted = po.muted.clone();
        }
        s.cpu_custom_name = po.cpu_label.clone();
        s.gpu_custom_name = po.gpu_label.clone();
        let p = Palette::from_settings(&s, po.opacity);

        let mut tiles: Vec<Element<'_, Message>> = Vec::new();
        if po.show_cpu { tiles.push(tile::cpu_tile(&snap.cpu, &s, p, warn_view_for(&po.warnings, "CPU", &snap, self.flash_on), true)); }
        if po.show_gpu { tiles.push(tile::gpu_tile(&snap.gpu, &s, p, warn_view_for(&po.warnings, "GPU", &snap, self.flash_on))); }
        if po.show_ram { tiles.push(tile::ram_tile(&snap.ram, &s, p, warn_view_for(&po.warnings, "RAM", &snap, self.flash_on))); }
        if po.show_network { tiles.push(tile::network_tile(&snap.network, &s, p, no_warn, 1.0)); }
        if po.show_storage { tiles.push(tile::disk_tile(&snap.disk, &s, p, no_warn)); }
        let skin = style::skin_style(&s.active_skin);
        let body = column(tiles).spacing(skin.tile_spacing).align_x(iced::Alignment::Center);

        let label = if connected { name } else { format!("{name}  \u{2022} offline") };
        let header = row![
            Space::with_width(Length::Fill),
            text(label).size(9)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
            Space::with_width(Length::Fill),
            button(text("\u{2715}").size(11).font(iced::Font::with_name("Segoe UI Symbol"))
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding(0).style(|_, _| button::Style { background: None, ..Default::default() })
                .on_press(Message::ClosePopup(id)),
        ].align_y(iced::Alignment::Center).height(16);

        let widget_border = skin.border_color(&p);
        let root = container(column![header, Space::with_height(4), body])
            .width(Length::Fill).height(Length::Fill).padding(6)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(p.bg)),
                border: Border { radius: 12.0.into(), width: skin.widget_border, color: widget_border },
                ..Default::default()
            });
        mouse_area(root).on_press(Message::DragWindow(id)).into()
    }

    // The device switcher strip: "This PC" + one tab per remote device, each
    // with an optional green/red status dot. Centered above the tiles.
    fn device_tabs(&self, active_id: Option<&String>, p: Palette) -> Element<'_, Message> {
        let show_dot = self.settings.show_remote_status_dot;
        let make_tab = move |label: String, active: bool, dot: Option<bool>, msg: Message| -> Element<'static, Message> {
            let mut content = row![].spacing(4).align_y(iced::Alignment::Center);
            if let (true, Some(connected)) = (show_dot, dot) {
                let c = if connected { Color::from_rgb(0.30, 0.78, 0.45) } else { Color::from_rgb(0.86, 0.30, 0.25) };
                content = content.push(status_dot(c));
            }
            content = content.push(
                text(label).size(10)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(if active { Color::WHITE } else { p.muted }) })
            );
            button(content)
                .padding(iced::Padding { top: 2.0, right: 8.0, bottom: 2.0, left: 8.0 })
                .style(move |_: &iced::Theme, status: button::Status| {
                    let hover = matches!(status, button::Status::Hovered);
                    button::Style {
                        background: Some(iced::Background::Color(if active { p.accent } else if hover { p.tile } else { Color { a: p.tile.a * 0.5, ..p.tile } })),
                        text_color: if active { Color::WHITE } else { p.muted },
                        border: Border { radius: 5.0.into(), ..Border::default() },
                        ..Default::default()
                    }
                })
                .on_press(msg).into()
        };

        let mut strip = row![].spacing(3).align_y(iced::Alignment::Center);
        strip = strip.push(style::with_tip(
            make_tab("This PC".into(), active_id.is_none(), None, Message::SwitchWidgetDevice(None)),
            "Show this PC's sensors", p));
        for d in &self.settings.remote_devices {
            let connected = self.remote_conn.get(&d.id).copied().unwrap_or(false);
            let active = active_id == Some(&d.id);
            let tip = format!("{} \u{2014} {}", d.name, if connected { "connected" } else { "disconnected" });
            strip = strip.push(style::with_tip(
                make_tab(truncate(&d.name, 12), active, Some(connected), Message::SwitchWidgetDevice(Some(d.id.clone()))),
                &tip, p));
        }
        container(strip).width(Length::Fill).center_x(Length::Fill).into()
    }

    fn widget_view(&self, id: window::Id, p: Palette) -> Element<'_, Message> {
        let pulse = self.traffic_pulse();

        // Which device's data are we showing? None = this PC.
        let active_id: Option<&String> = self.widget_device.as_ref()
            .filter(|id| self.settings.remote_devices.iter().any(|d| &d.id == *id));
        let is_remote = active_id.is_some();
        let snap_owned: SensorSnapshot = match active_id {
            Some(rid) => self.remote_snapshots.get(rid).cloned().unwrap_or_default(),
            None => self.snapshot.clone(),
        };
        let snap = &snap_owned;
        let dev = active_id.and_then(|rid| self.settings.remote_devices.iter().find(|d| &d.id == rid));
        let remote_warns: &[fluid_core::settings::TileWarning] =
            dev.map(|d| d.popout.warnings.as_slice()).unwrap_or(&[]);

        let mut tiles: Vec<Element<'_, Message>> = Vec::new();
        for name in self.current_tiles() {
            let w = if is_remote {
                warn_view_for(remote_warns, &name, snap, self.flash_on)
            } else {
                self.warn_view(&name)
            };
            // The "turn on temp" hint is about the LOCAL driver, so suppress it
            // on a remote view by passing driver_installed = true.
            let cpu_driver = is_remote || self.cpu_driver_installed;
            let el = match name.as_str() {
                "CPU" => tile::cpu_tile(&snap.cpu, &self.settings, p, w, cpu_driver),
                "GPU" => tile::gpu_tile(&snap.gpu, &self.settings, p, w),
                "RAM" => tile::ram_tile(&snap.ram, &self.settings, p, w),
                "Disk" => tile::disk_tile(&snap.disk, &self.settings, p, w),
                "Network" => tile::network_tile(&snap.network, &self.settings, p, w, pulse),
                "Clock" => tile::clock_tile(&self.settings, p, w),
                _ => continue,
            };
            tiles.push(el);
        }
        let skin = style::skin_style(&self.settings.active_skin);
        let body: Element<'_, Message> = match self.effective_orientation() {
            Orientation::Vertical => column(tiles).spacing(skin.tile_spacing).into(),
            Orientation::Horizontal => row(tiles).spacing(skin.tile_spacing).into(),
        };
        let icon_btn = |label: &str, sz: u16, msg: Message| {
            button(text(label.to_string()).size(sz).font(iced::Font::with_name("Segoe UI Symbol"))
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
            ).padding(0).style(|_, _| button::Style { background: None, ..Default::default() }).on_press(msg)
        };
        let header = row![
            style::with_tip(icon_btn("\u{2699}", 15, Message::OpenSettings), "Open settings", p),
            Space::with_width(Length::Fill),
            style::with_tip(icon_btn("\u{2715}", 13, Message::HideWidget), "Hide the widget (stays running in the tray)", p),
        ].height(20);

        // Device switcher tabs (this PC + each remote), shown only with remotes.
        let widget_border = skin.border_color(&p);
        let mut shell = column![header];
        if self.show_device_tabs() {
            shell = shell.push(Space::with_height(2));
            shell = shell.push(self.device_tabs(active_id, p));
        }
        shell = shell.push(Space::with_height(4));
        shell = shell.push(body);
        // Bold skins glow the whole widget frame too (accent-tinted bloom).
        let frame_shadow = if skin.glow > 0.0 {
            iced::Shadow { color: Color { a: 0.45 * skin.glow, ..p.accent }, offset: iced::Vector::new(0.0, 0.0), blur_radius: 8.0 + skin.glow * 18.0 }
        } else {
            iced::Shadow::default()
        };
        let root = container(shell)
            .width(Length::Fill).height(Length::Fill).padding(8)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(p.bg)),
                border: Border { radius: skin.widget_radius.into(), width: skin.widget_border, color: widget_border },
                shadow: frame_shadow,
                ..Default::default()
            });
        mouse_area(root)
            .on_press(Message::DragWindow(id))
            .on_release(Message::SnapWidgetNow)
            .on_right_press(Message::ShowWidgetMenu)
            .into()
    }

    // Opacity multiplier for the network arrows.
    //   Blink = quick fade in/out (~0.7s), Fade = very slow fade (~3s),
    //   Glow = static (handled with a glow halo in the tile), Off = 1.0.
    fn traffic_pulse(&self) -> f32 {
        let tau = std::f32::consts::TAU;
        let wave = |period: f32| ((self.anim_t / period) * tau).sin() * 0.5 + 0.5;
        match self.settings.network_traffic_indicator.as_str() {
            "Blink" => 0.30 + 0.70 * wave(0.7),
            "Fade" => 0.15 + 0.85 * wave(3.0),
            _ => 1.0,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            iced::time::every(Duration::from_millis(self.settings.update_interval_ms.max(250))).map(|_| Message::SensorTick),
            iced::time::every(Duration::from_millis(200)).map(|_| Message::TrayPoll),
            iced::time::every(Duration::from_millis(250)).map(|_| Message::RemotePoll),
            iced::time::every(Duration::from_millis(600)).map(|_| Message::FlashTick),
            window::close_events().map(Message::WindowClosed),
            window::events().map(|(id, event)| match event {
                window::Event::Moved(pos) => Message::WindowMoved(id, pos),
                window::Event::Unfocused => Message::WindowUnfocused(id),
                _ => Message::TrayPoll,
            }),
        ];
        // Only run the animation clock for the animated modes (Glow is static).
        if matches!(self.settings.network_traffic_indicator.as_str(), "Blink" | "Fade") {
            subs.push(iced::time::every(Duration::from_millis(60)).map(|_| Message::AnimTick));
        }
        // While a hotkey field is armed, capture the next key combo.
        if self.capturing_hotkey.is_some() {
            subs.push(iced::keyboard::on_key_press(|key, mods| Some(Message::HotkeyKeyPressed(key, mods))));
        }
        Subscription::batch(subs)
    }
    fn theme(&self, _id: window::Id) -> Theme { Theme::Dark }

    fn title(&self, id: window::Id) -> String {
        match self.windows.get(&id) {
            Some(WindowKind::Widget) => WIDGET_TITLE.to_string(),
            _ => DEFAULT_TITLE.to_string(),
        }
    }
}
