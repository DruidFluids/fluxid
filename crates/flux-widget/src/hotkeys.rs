//! Global hotkeys (Windows). winit owns the widget's message loop, so we run a
//! dedicated thread with its own loop that calls `RegisterHotKey(None, ...)` —
//! WM_HOTKEY is then posted to *this thread's* queue. Re-registration is driven
//! by posting WM_APP back to the thread. Mirrors the C# MainWindow hotkey code.

#[cfg(target_os = "windows")]
pub use imp::*;

/// Which action a captured/registered hotkey triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyTarget {
    ClickThrough,
    GameMode,
}

/// An event emitted when a registered hotkey fires.
#[derive(Debug, Clone, Copy)]
pub enum HotkeyEvent {
    ClickThrough,
    GameMode,
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{HotkeyEvent, HotkeyTarget};
    use std::sync::{Arc, Mutex};
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
        MOD_SHIFT, MOD_WIN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY, WM_USER,
    };

    const ID_CLICK_THROUGH: i32 = 0x9001;
    const ID_GAME_MODE: i32 = 0x9002;
    const WM_APP_REREGISTER: u32 = WM_APP;

    struct Shared {
        click: String,
        game: String,
    }

    /// Owns the hotkey thread. Drop unregisters by ending the process; the
    /// widget lives for the whole session so explicit teardown isn't needed.
    pub struct HotkeyManager {
        thread_id: u32,
        shared: Arc<Mutex<Shared>>,
    }

    impl HotkeyManager {
        pub fn start() -> (HotkeyManager, std::sync::mpsc::Receiver<HotkeyEvent>) {
            let shared = Arc::new(Mutex::new(Shared { click: String::new(), game: String::new() }));
            let (ev_tx, ev_rx) = std::sync::mpsc::channel();
            let (tid_tx, tid_rx) = std::sync::mpsc::channel();
            let shared_thread = shared.clone();

            std::thread::Builder::new()
                .name("flux-hotkeys".into())
                .spawn(move || unsafe {
                    // Force the thread message queue to exist before anyone posts
                    // to it (PostThreadMessage drops messages otherwise).
                    let mut msg = MSG::default();
                    let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
                    let _ = tid_tx.send(GetCurrentThreadId());

                    loop {
                        let r = GetMessageW(&mut msg, None, 0, 0);
                        if r.0 == 0 || r.0 == -1 {
                            break; // WM_QUIT or error
                        }
                        match msg.message {
                            WM_HOTKEY => {
                                let id = msg.wParam.0 as i32;
                                if id == ID_CLICK_THROUGH {
                                    let _ = ev_tx.send(HotkeyEvent::ClickThrough);
                                } else if id == ID_GAME_MODE {
                                    let _ = ev_tx.send(HotkeyEvent::GameMode);
                                }
                            }
                            WM_APP_REREGISTER => {
                                let (c, g) = {
                                    let s = shared_thread.lock().unwrap();
                                    (s.click.clone(), s.game.clone())
                                };
                                let _ = UnregisterHotKey(None, ID_CLICK_THROUGH);
                                let _ = UnregisterHotKey(None, ID_GAME_MODE);
                                if let Some((m, vk)) = parse_combo(&c) {
                                    let _ = RegisterHotKey(None, ID_CLICK_THROUGH, m, vk);
                                }
                                if let Some((m, vk)) = parse_combo(&g) {
                                    let _ = RegisterHotKey(None, ID_GAME_MODE, m, vk);
                                }
                            }
                            _ => {}
                        }
                    }
                })
                .expect("spawn hotkey thread");

            let thread_id = tid_rx.recv().unwrap_or(0);
            (HotkeyManager { thread_id, shared }, ev_rx)
        }

        pub fn set_combo(&self, target: HotkeyTarget, combo: &str) {
            {
                let mut s = self.shared.lock().unwrap();
                match target {
                    HotkeyTarget::ClickThrough => s.click = combo.to_string(),
                    HotkeyTarget::GameMode => s.game = combo.to_string(),
                }
            }
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_APP_REREGISTER, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Parse a combo string ("Ctrl+Shift+F12") into Win32 modifier flags + VK.
    fn parse_combo(combo: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
        if combo.is_empty() {
            return None;
        }
        let mut bits = MOD_NOREPEAT.0;
        let mut vk = None;
        for part in combo.split('+') {
            match part.trim().to_ascii_uppercase().as_str() {
                "CTRL" | "CONTROL" => bits |= MOD_CONTROL.0,
                "ALT" => bits |= MOD_ALT.0,
                "SHIFT" => bits |= MOD_SHIFT.0,
                "WIN" | "WINDOWS" => bits |= MOD_WIN.0,
                other => vk = key_name_to_vk(other),
            }
        }
        let vk = vk?;
        if vk == 0 {
            return None;
        }
        Some((HOT_KEY_MODIFIERS(bits), vk))
    }

    /// Map an uppercase key name to a Windows virtual-key code.
    fn key_name_to_vk(name: &str) -> Option<u32> {
        if name.len() == 1 {
            let c = name.as_bytes()[0];
            if c.is_ascii_alphabetic() || c.is_ascii_digit() {
                return Some(c as u32); // 'A'..'Z' = 0x41.., '0'..'9' = 0x30..
            }
        }
        if let Some(num) = name.strip_prefix('F') {
            if let Ok(k) = num.parse::<u32>() {
                if (1..=24).contains(&k) {
                    return Some(0x70 + k - 1); // VK_F1 = 0x70
                }
            }
        }
        Some(match name {
            "SPACE" => 0x20,
            "ENTER" | "RETURN" => 0x0D,
            "TAB" => 0x09,
            "INSERT" => 0x2D,
            "DELETE" => 0x2E,
            "HOME" => 0x24,
            "END" => 0x23,
            "PAGEUP" => 0x21,
            "PAGEDOWN" => 0x22,
            "LEFT" => 0x25,
            "UP" => 0x26,
            "RIGHT" => 0x27,
            "DOWN" => 0x28,
            "BACKSPACE" => 0x08,
            _ => return None,
        })
    }
}

/// Format an iced key + modifiers into a combo string ("Ctrl+Shift+F12"), or
/// `None` if the key isn't usable as a hotkey (bare modifier / unsupported).
pub fn format_combo(key: &iced::keyboard::Key, mods: iced::keyboard::Modifiers) -> Option<String> {
    let name = key_display_name(key)?;
    let mut parts: Vec<String> = Vec::new();
    if mods.control() {
        parts.push("Ctrl".into());
    }
    if mods.alt() {
        parts.push("Alt".into());
    }
    if mods.shift() {
        parts.push("Shift".into());
    }
    if mods.logo() {
        parts.push("Win".into());
    }
    parts.push(name);
    Some(parts.join("+"))
}

fn key_display_name(key: &iced::keyboard::Key) -> Option<String> {
    use iced::keyboard::key::Named;
    use iced::keyboard::Key;
    match key {
        Key::Character(s) => {
            let c = s.chars().next()?;
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_uppercase().to_string())
            } else {
                None
            }
        }
        Key::Named(named) => {
            let n = match named {
                Named::Space => "Space",
                Named::Enter => "Enter",
                Named::Tab => "Tab",
                Named::Backspace => "Backspace",
                Named::Delete => "Delete",
                Named::Insert => "Insert",
                Named::Home => "Home",
                Named::End => "End",
                Named::PageUp => "PageUp",
                Named::PageDown => "PageDown",
                Named::ArrowUp => "Up",
                Named::ArrowDown => "Down",
                Named::ArrowLeft => "Left",
                Named::ArrowRight => "Right",
                Named::F1 => "F1",
                Named::F2 => "F2",
                Named::F3 => "F3",
                Named::F4 => "F4",
                Named::F5 => "F5",
                Named::F6 => "F6",
                Named::F7 => "F7",
                Named::F8 => "F8",
                Named::F9 => "F9",
                Named::F10 => "F10",
                Named::F11 => "F11",
                Named::F12 => "F12",
                _ => return None,
            };
            Some(n.to_string())
        }
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
pub struct HotkeyManager;

#[cfg(not(target_os = "windows"))]
impl HotkeyManager {
    pub fn start() -> (HotkeyManager, std::sync::mpsc::Receiver<HotkeyEvent>) {
        let (_tx, rx) = std::sync::mpsc::channel();
        (HotkeyManager, rx)
    }
    pub fn set_combo(&self, _target: HotkeyTarget, _combo: &str) {}
}
