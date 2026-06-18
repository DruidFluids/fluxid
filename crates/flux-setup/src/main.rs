//! Flux setup — a self-contained custom installer.
//!
//! Three modes, chosen by CLI args:
//! * (no args) → the iced wizard GUI.
//! * `--apply` → headless install engine (also the elevated worker the GUI
//!   spawns for an all-users install).
//! * `--uninstall` → headless uninstall engine; this exe is copied into the
//!   install dir and registered as the Add/Remove-Programs uninstall command.
//!
//! The widget (`flux.exe`) is embedded at build time (see `build.rs` /
//! `payload.rs`); there is no separate service and no runtime dependency, so
//! the installer's whole job is: copy the exe, make shortcuts, register the
//! uninstaller, apply the startup opt-in, and launch.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod engine;
mod payload;
mod style;

use engine::{InstallOptions, Scope, UninstallOptions};

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if cli::has(&args, &["help", "h", "?"]) {
        show_help();
        std::process::exit(0);
    }
    if cli::has(&args, &["uninstall"]) {
        // Silent/quiet uninstall (QuietUninstallString) stays headless; an
        // interactive uninstall opens the same wizard GUI in uninstall mode.
        if cli::is_silent(&args) {
            std::process::exit(run_uninstall_cli(&args));
        }
        return gui::run();
    }
    // A silent/quiet switch on its own (no GUI) implies a headless install with
    // default options — the NSIS-style `/S` convention.
    if cli::has(&args, &["apply", "install"]) || cli::is_silent(&args) {
        std::process::exit(run_apply_cli(&args));
    }

    gui::run()
}

/// Argument parsing shared by every mode.
///
/// Every flag accepts `--flag`, `-flag` and `/flag` (case-insensitive); the few
/// that take a value use a following argument (`--scope all-users`). This keeps
/// one source of truth so each installer feature has exactly one switch.
mod cli {
    use super::Scope;

    /// Strip any leading `-`/`/` and lowercase, so `/S`, `-s`, `--silent` all
    /// normalise to a comparable token.
    fn norm(a: &str) -> String {
        a.trim_start_matches(['-', '/']).to_ascii_lowercase()
    }

    /// True if any arg matches one of `names` (already-normalised, lowercase).
    pub fn has(args: &[String], names: &[&str]) -> bool {
        args.iter().any(|a| {
            let n = norm(a);
            names.iter().any(|name| n == *name)
        })
    }

    /// The argument following the first occurrence of any of `names`.
    pub fn value<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
        let i = args.iter().position(|a| {
            let n = norm(a);
            names.iter().any(|name| n == *name)
        })?;
        args.get(i + 1).map(|s| s.as_str())
    }

    /// `/S`, `/q`, `--silent`, `--quiet` — suppress UI / message boxes.
    pub fn is_silent(args: &[String]) -> bool {
        has(args, &["s", "silent", "q", "quiet"])
    }

    pub fn scope(args: &[String]) -> Scope {
        value(args, &["scope"])
            .and_then(Scope::parse)
            .unwrap_or(Scope::PerUser)
    }
}

// ── Headless modes ──

/// Headless install. Used both directly (silent/scripted installs) and as the
/// elevated worker the GUI relaunches for an all-users install.
///
/// Default = install everything (desktop + startup + launch); opt out with
/// `--no-desktop` / `--no-startup` / `--no-launch`. The GUI always passes an
/// explicit set of flags so there is no ambiguity for the elevated worker — and
/// it passes `--no-launch` so the worker never starts the widget elevated.
fn run_apply_cli(args: &[String]) -> i32 {
    // If the CPU-sensor service is INSTALLED, the update must manage it under
    // elevation: a running service holds flux.exe open (overwrite would fail), and
    // even a stopped-but-registered service has to be (re)started afterward — and
    // stopping/starting a LocalSystem service requires admin. So relaunch elevated
    // (one UAC) whenever the service exists. Already-elevated workers (and machines
    // without the service) fall straight through.
    if engine::sensor_service_exists() && !engine::is_elevated() {
        return match engine::relaunch_elevated_wait(args) {
            Ok(Some(code)) => code,
            Ok(None) => 1, // user declined the UAC prompt
            Err(_) => 1,
        };
    }
    let opts = InstallOptions {
        scope: cli::scope(args),
        desktop_shortcut: !cli::has(args, &["no-desktop", "nodesktop"]),
        run_at_startup: !cli::has(args, &["no-startup", "nostartup"]),
        launch_after: !cli::has(args, &["no-launch", "nolaunch"]),
    };
    let silent = cli::is_silent(args);
    match engine::install(opts) {
        Ok(_) => 0,
        Err(e) => {
            if !silent {
                msgbox(&format!("Install failed:\n\n{e}"), "Flux Setup", true);
            }
            1
        }
    }
}

fn run_uninstall_cli(args: &[String]) -> i32 {
    let opts = UninstallOptions {
        scope: cli::scope(args),
        remove_settings: cli::has(args, &["remove-settings", "removesettings"]),
    };
    let silent = cli::is_silent(args);
    match engine::uninstall(opts) {
        Ok(_) => {
            if !silent {
                msgbox("Flux has been uninstalled.", "Flux", false);
            }
            0
        }
        Err(e) => {
            if !silent {
                msgbox(&format!("Uninstall failed:\n\n{e}"), "Flux", true);
            }
            1
        }
    }
}

const HELP_TEXT: &str = "\
Flux Setup — command-line switches

  (no switches)         Launch the graphical setup wizard.

Modes
  --install, --apply    Install without the wizard (headless).
  --uninstall           Uninstall (this is what Add/Remove Programs calls).
  /S, /q, --silent      Silent: no wizard and no message boxes. On its own,
                        runs a headless install with default options.
  --help, /?            Show this help.

Install options (default: install everything, per-user)
  --scope per-user      Install for the current user (no admin). Default.
  --scope all-users     Install for all users (prompts for administrator).
  --no-desktop          Do not create a desktop shortcut.
  --no-startup          Do not start Flux with Windows.
  --no-launch           Do not launch Flux when setup finishes.
  --all                 Enable every optional feature (the default).

Uninstall options
  --scope <type>        Match the scope Flux was installed with.
  --remove-settings     Also delete %APPDATA%\\Flux (settings/themes/skins).
  /S, --silent          Uninstall with no message boxes.

Every switch accepts --flag, -flag or /flag (case-insensitive).";

fn show_help() {
    // Console for dev/debug builds; a message box for the windowed release.
    println!("{HELP_TEXT}");
    #[cfg(all(windows, not(debug_assertions)))]
    msgbox(HELP_TEXT, "Flux Setup", false);
}

#[cfg(windows)]
fn msgbox(text: &str, caption: &str, error: bool) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
    };
    let icon = if error { MB_ICONERROR } else { MB_ICONINFORMATION };
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from(caption),
            MB_OK | icon,
        );
    }
}
#[cfg(not(windows))]
fn msgbox(_text: &str, _caption: &str, _error: bool) {}

// ───────────────────────────── GUI wizard ─────────────────────────────

mod gui {
    use super::*;
    use iced::widget::{
        button, checkbox, column, container, radio, row, scrollable, text, Space,
    };
    use iced::{Alignment, Element, Length, Task, Theme};

    pub fn run() -> iced::Result {
        let window = iced::window::Settings {
            size: iced::Size::new(500.0, 500.0),
            min_size: Some(iced::Size::new(470.0, 470.0)),
            icon: load_icon(),
            ..iced::window::Settings::default()
        };
        let mut app = iced::application("Flux Setup", Wizard::update, Wizard::view)
            .theme(Wizard::theme)
            .window(window);
        // Segoe UI Symbol gives us the ✓ glyph (iced's default font lacks it),
        // matching how the widget loads it for its monochrome icons.
        #[cfg(target_os = "windows")]
        if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\seguisym.ttf") {
            app = app.font(bytes);
        }
        app.run_with(Wizard::new)
    }

    /// Font that carries the ✓ glyph used in the Done checklist.
    const SYMBOL_FONT: iced::Font = iced::Font::with_name("Segoe UI Symbol");

    /// Decode the bundled Flux logo for the window / taskbar icon (same PNG
    /// the widget uses), so setup is visually branded as Flux.
    fn load_icon() -> Option<iced::window::Icon> {
        const PNG: &[u8] = include_bytes!("../assets/icon.png");
        let img = image::load_from_memory(PNG).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
    }

    #[derive(Debug, Clone)]
    pub enum Message {
        Next,
        Back,
        SetScope(Scope),
        ToggleDesktop(bool),
        ToggleStartup(bool),
        ToggleLaunch(bool),
        StartInstall,
        Installed(Outcome),
        ToggleRemoveSettings(bool),
        StartUninstall,
        Uninstalled(Outcome),
        Finish,
    }

    /// A Clone+Send result the async install Task hands back to the UI.
    #[derive(Debug, Clone)]
    pub struct Outcome {
        pub ok: bool,
        pub steps: Vec<String>,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Install,
        Uninstall,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Page {
        Welcome,
        Options,
        Installing,
        Done,
        // Uninstall-mode first page: confirm + "remove all" checkbox.
        ConfirmUninstall,
    }

    struct Wizard {
        mode: Mode,
        page: Page,
        scope: Scope,
        desktop: bool,
        startup: bool,
        launch: bool,
        /// Uninstall: also delete settings/themes/skins + sensor service + driver.
        remove_settings: bool,
        outcome: Option<Outcome>,
        qa: bool,
    }

    impl Wizard {
        fn new() -> (Self, Task<Message>) {
            // Hidden `--page <welcome|options|installing|done>` to open the
            // wizard on a given page (used for screenshots / visual QA).
            let args: Vec<String> = std::env::args().collect();
            // QA screenshot mode: when launched via the hidden --page flag, show
            // placeholder paths (C:\Users\you\...) instead of the real install dir
            // so captured screenshots never leak the local Windows username.
            let qa = crate::cli::value(&args, &["page"]).is_some();
            // Uninstall mode (invoked by Add/Remove Programs as `--uninstall`):
            // open straight to the confirm page; the scope rides in on the flag.
            let uninstall = crate::cli::has(&args[1..], &["uninstall"]);
            let (page, outcome) = match crate::cli::value(&args, &["page"]) {
                Some("options") => (Page::Options, None),
                Some("installing") => (Page::Installing, None),
                Some("done") => (
                    Page::Done,
                    Some(Outcome {
                        ok: true,
                        steps: vec![
                            "Created C:\\Users\\you\\AppData\\Local\\Flux".into(),
                            "Installed flux.exe".into(),
                            "Installed uninstaller".into(),
                            "Created Start Menu shortcut".into(),
                            "Created desktop shortcut".into(),
                            "Registered in Add/Remove Programs".into(),
                            "Enabled start with Windows".into(),
                            "Launched Flux".into(),
                        ],
                        error: None,
                    }),
                ),
                _ if uninstall => (Page::ConfirmUninstall, None),
                _ => (Page::Welcome, None),
            };
            (
                Self {
                    mode: if uninstall { Mode::Uninstall } else { Mode::Install },
                    page,
                    scope: crate::cli::scope(&args[1..]),
                    desktop: true,
                    startup: true,
                    launch: true,
                    remove_settings: true, // "remove all traces" on by default
                    outcome,
                    qa,
                },
                Task::none(),
            )
        }

        fn options(&self) -> InstallOptions {
            InstallOptions {
                scope: self.scope,
                desktop_shortcut: self.desktop,
                run_at_startup: self.startup,
                launch_after: self.launch,
            }
        }

        fn update(&mut self, message: Message) -> Task<Message> {
            match message {
                Message::Next => {
                    self.page = Page::Options;
                    Task::none()
                }
                Message::Back => {
                    self.page = Page::Welcome;
                    Task::none()
                }
                Message::SetScope(s) => {
                    self.scope = s;
                    Task::none()
                }
                Message::ToggleDesktop(v) => {
                    self.desktop = v;
                    Task::none()
                }
                Message::ToggleStartup(v) => {
                    self.startup = v;
                    Task::none()
                }
                Message::ToggleLaunch(v) => {
                    self.launch = v;
                    Task::none()
                }
                Message::StartInstall => {
                    self.page = Page::Installing;
                    let opts = self.options();
                    Task::perform(run_install_async(opts), Message::Installed)
                }
                Message::Installed(outcome) => {
                    self.outcome = Some(outcome);
                    self.page = Page::Done;
                    Task::none()
                }
                Message::ToggleRemoveSettings(v) => {
                    self.remove_settings = v;
                    Task::none()
                }
                Message::StartUninstall => {
                    self.page = Page::Installing;
                    let opts = UninstallOptions {
                        scope: self.scope,
                        remove_settings: self.remove_settings,
                    };
                    Task::perform(run_uninstall_async(opts), Message::Uninstalled)
                }
                Message::Uninstalled(outcome) => {
                    self.outcome = Some(outcome);
                    self.page = Page::Done;
                    Task::none()
                }
                Message::Finish => iced::exit(),
            }
        }

        fn view(&self) -> Element<'_, Message> {
            // Install is a 4-step flow (Welcome/Options/Installing/Done); uninstall
            // is 3 (Confirm/Uninstalling/Done). The step bar tracks whichever.
            let total = match self.mode {
                Mode::Install => 4,
                Mode::Uninstall => 3,
            };
            let step = match (self.mode, self.page) {
                (Mode::Install, Page::Welcome) => 0,
                (Mode::Install, Page::Options) => 1,
                (Mode::Install, Page::Installing) => 2,
                (Mode::Install, Page::Done) => 3,
                (Mode::Uninstall, Page::ConfirmUninstall) => 0,
                (Mode::Uninstall, Page::Installing) => 1,
                (Mode::Uninstall, Page::Done) => 2,
                _ => 0,
            };
            let (content, buttons) = match self.page {
                Page::Welcome => self.welcome(),
                Page::Options => self.options_page(),
                Page::ConfirmUninstall => self.confirm_uninstall(),
                Page::Installing => self.installing(),
                Page::Done => self.done(),
            };
            // The options page is content-dense — pin it to the top so nothing
            // is clipped; the other (short) pages look best vertically centered.
            let center = !matches!(self.page, Page::Options);
            frame(step, total, content, buttons, center)
        }

        fn welcome(&self) -> (Element<'_, Message>, Element<'_, Message>) {
            let note: Element<'_, Message> = if payload::is_bundled() {
                text(format!("Package size: {:.1} MB", payload::size_mb()))
                    .size(12)
                    .style(style::muted)
                    .into()
            } else {
                text("Development build — no payload bundled; install is disabled.")
                    .size(12)
                    .style(style::danger)
                    .into()
            };
            let content = column![
                style::badge(),
                Space::with_height(12),
                text("Flux").size(28).style(style::heading),
                text(format!(
                    "v{} — system monitor widget for Windows",
                    engine::VERSION
                ))
                .size(14)
                .style(style::body),
                text("Personal Use License — source-available")
                    .size(13)
                    .style(style::muted),
                Space::with_height(6),
                note,
            ]
            .spacing(6)
            .align_x(Alignment::Center);

            let buttons = row![
                secondary_button("Cancel", Some(Message::Finish)),
                primary_button("Next  →", payload::is_bundled().then_some(Message::Next)),
            ]
            .spacing(12);

            (content.into(), buttons.into())
        }

        fn options_page(&self) -> (Element<'_, Message>, Element<'_, Message>) {
            let location: Element<'_, Message> = if self.qa {
                // Screenshot mode: generic path, never the real username.
                let demo = match self.scope {
                    Scope::AllUsers => "C:\\Program Files\\Flux",
                    Scope::PerUser => "C:\\Users\\you\\AppData\\Local\\Flux",
                };
                text(format!("Location: {demo}")).size(12).style(style::muted).into()
            } else {
                match engine::install_dir(self.scope) {
                    Ok(dir) => text(format!("Location: {}", dir.display()))
                        .size(12)
                        .style(style::muted)
                        .into(),
                    Err(_) => Space::with_height(0).into(),
                }
            };

            let elevation_note: Element<'_, Message> = if self.scope == Scope::AllUsers {
                text("You'll be asked to approve a Windows admin prompt.")
                    .size(12)
                    .style(style::muted)
                    .into()
            } else {
                Space::with_height(0).into()
            };

            let content = column![
                text("Setup options").size(22).style(style::heading),
                Space::with_height(6),
                text("Install for").size(14).style(style::muted),
                radio(
                    "Just me  (no admin required)",
                    Scope::PerUser,
                    Some(self.scope),
                    Message::SetScope,
                ),
                radio(
                    "All users  (requires administrator)",
                    Scope::AllUsers,
                    Some(self.scope),
                    Message::SetScope,
                ),
                location,
                elevation_note,
                Space::with_height(6),
                text("Options").size(14).style(style::muted),
                checkbox("Create a desktop shortcut", self.desktop)
                    .on_toggle(Message::ToggleDesktop),
                checkbox("Start Flux when Windows starts", self.startup)
                    .on_toggle(Message::ToggleStartup),
                checkbox("Launch Flux when setup finishes", self.launch)
                    .on_toggle(Message::ToggleLaunch),
            ]
            .spacing(9)
            .width(Length::Fixed(380.0));

            let buttons = row![
                secondary_button("←  Back", Some(Message::Back)),
                primary_button("Install", Some(Message::StartInstall)),
            ]
            .spacing(12);

            (content.into(), buttons.into())
        }

        fn confirm_uninstall(&self) -> (Element<'_, Message>, Element<'_, Message>) {
            let note = if self.remove_settings {
                "Complete removal — your settings/themes/skins, the sensor service, and the optional PawnIO driver are all removed."
            } else {
                "Your settings, themes & skins (and the sensor service) will be kept for a future reinstall."
            };
            let content = column![
                style::badge(),
                Space::with_height(12),
                text("Uninstall Flux").size(22).style(style::heading),
                text(format!("v{} — remove Flux from this PC.", engine::VERSION))
                    .size(14)
                    .style(style::muted),
                Space::with_height(12),
                container(
                    checkbox("Also remove my settings, themes & skins", self.remove_settings)
                        .on_toggle(Message::ToggleRemoveSettings),
                )
                .width(Length::Fixed(340.0)),
                container(text(note).size(12).style(style::muted))
                    .width(Length::Fixed(340.0)),
            ]
            .spacing(6)
            .align_x(Alignment::Center);

            let buttons = row![
                secondary_button("Cancel", Some(Message::Finish)),
                primary_button("Uninstall", Some(Message::StartUninstall)),
            ]
            .spacing(12);

            (content.into(), buttons.into())
        }

        fn installing(&self) -> (Element<'_, Message>, Element<'_, Message>) {
            let (title, sub) = match self.mode {
                Mode::Install => ("Installing…", "Setting up Flux. This only takes a moment."),
                Mode::Uninstall => ("Uninstalling…", "Removing Flux. This only takes a moment."),
            };
            let content = column![
                style::badge(),
                Space::with_height(14),
                text(title).size(22).style(style::heading),
                text(sub).size(14).style(style::muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center);

            (content.into(), Space::with_height(0).into())
        }

        fn done(&self) -> (Element<'_, Message>, Element<'_, Message>) {
            let content: Element<'_, Message> = match &self.outcome {
                Some(o) if o.ok => {
                    let steps = o.steps.iter().fold(column![].spacing(7), |c, s| {
                        c.push(
                            row![
                                text("✓")
                                    .font(SYMBOL_FONT)
                                    .size(14)
                                    .style(style::accent_text),
                                text(s).size(13).style(style::body),
                            ]
                            .spacing(10)
                            .align_y(Alignment::Center),
                        )
                    });
                    column![
                        text(if self.mode == Mode::Uninstall { "Uninstall complete" } else { "Setup complete" })
                            .size(22).style(style::heading),
                        Space::with_height(14),
                        container(scrollable(steps)).width(Length::Fixed(330.0)),
                    ]
                    .spacing(4)
                    .align_x(Alignment::Center)
                    .into()
                }
                Some(o) => column![
                    text(if self.mode == Mode::Uninstall { "Uninstall failed" } else { "Setup failed" })
                        .size(22).style(style::heading),
                    Space::with_height(12),
                    text(o.error.clone().unwrap_or_else(|| "Unknown error.".into()))
                        .size(14)
                        .style(style::danger),
                ]
                .spacing(4)
                .align_x(Alignment::Center)
                .into(),
                None => Space::with_height(0).into(),
            };

            let buttons = row![primary_button("Close", Some(Message::Finish))];

            (content, buttons.into())
        }

        fn theme(&self) -> Theme {
            style::theme()
        }
    }

    /// Assemble a page: step indicator on top, centered content, divider, then a
    /// centered button row — the consistent wizard frame.
    fn frame<'a>(
        step: usize,
        total: usize,
        content: Element<'a, Message>,
        buttons: Element<'a, Message>,
        center: bool,
    ) -> Element<'a, Message> {
        let area = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill);
        let area = if center {
            area.center_y(Length::Fill)
        } else {
            area.align_y(iced::alignment::Vertical::Top)
        };
        container(
            column![
                step_bar(step, total),
                area,
                container(text("")).width(Length::Fill).height(Length::Fixed(1.0)).style(style::divider),
                container(buttons).width(Length::Fill).center_x(Length::Fill),
            ]
            .spacing(18),
        )
        .style(style::root)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(26)
        .into()
    }

    /// The row of step segments across the top (current + past are accent).
    fn step_bar(step: usize, total: usize) -> Element<'static, Message> {
        let mut bar = row![].spacing(8);
        for i in 0..total {
            bar = bar.push(
                container(text(""))
                    .width(Length::Fixed(34.0))
                    .height(Length::Fixed(5.0))
                    .style(style::segment(i <= step)),
            );
        }
        container(bar).width(Length::Fill).center_x(Length::Fill).into()
    }

    fn primary_button(label: &str, msg: Option<Message>) -> Element<'_, Message> {
        styled_button(label, msg, style::primary)
    }

    fn secondary_button(label: &str, msg: Option<Message>) -> Element<'_, Message> {
        styled_button(label, msg, style::secondary)
    }

    fn styled_button(
        label: &str,
        msg: Option<Message>,
        f: impl Fn(&Theme, button::Status) -> button::Style + 'static,
    ) -> Element<'_, Message> {
        let b = button(text(label).size(14))
            .padding([8, 22])
            .width(Length::Shrink)
            .style(f);
        match msg {
            Some(m) => b.on_press(m).into(),
            None => b.into(),
        }
    }

    /// Run the (blocking) install off the UI thread and normalise the result.
    async fn run_install_async(opts: InstallOptions) -> Outcome {
        let result =
            tokio::task::spawn_blocking(move || run_install_flow(opts)).await;
        match result {
            Ok(o) => o,
            Err(_) => Outcome {
                ok: false,
                steps: vec![],
                error: Some("Internal error during install.".into()),
            },
        }
    }

    /// Run the (blocking) uninstall off the UI thread. `engine::uninstall` handles
    /// its own elevation — one UAC prompt when "remove all traces" is enabled.
    async fn run_uninstall_async(opts: UninstallOptions) -> Outcome {
        let result = tokio::task::spawn_blocking(move || match engine::uninstall(opts) {
            Ok(rep) => Outcome { ok: true, steps: rep.steps, error: None },
            Err(e) => Outcome { ok: false, steps: vec![], error: Some(e.to_string()) },
        })
        .await;
        match result {
            Ok(o) => o,
            Err(_) => Outcome {
                ok: false,
                steps: vec![],
                error: Some("Internal error during uninstall.".into()),
            },
        }
    }

    /// Decide the in-process vs. elevated-worker path and produce an [`Outcome`].
    fn run_install_flow(opts: InstallOptions) -> Outcome {
        // Per-user (or already elevated) installs run in-process.
        if opts.scope == Scope::PerUser || engine::is_elevated() {
            return match engine::install(opts) {
                Ok(rep) => Outcome { ok: true, steps: rep.steps, error: None },
                Err(e) => Outcome {
                    ok: false,
                    steps: vec![],
                    error: Some(e.to_string()),
                },
            };
        }

        // All-users from an unelevated GUI: relaunch ourselves elevated to do
        // the privileged file/registry work, then launch the widget unelevated.
        // Pass an explicit, fully-specified flag set (the worker defaults to
        // "install everything", so the unchecked options must be negated), and
        // always --no-launch so the elevated worker never starts the widget.
        let mut apply = vec![
            "--apply".to_string(),
            "--scope".to_string(),
            "all-users".to_string(),
            "--no-launch".to_string(),
        ];
        if !opts.desktop_shortcut {
            apply.push("--no-desktop".into());
        }
        if !opts.run_at_startup {
            apply.push("--no-startup".into());
        }

        match engine::relaunch_elevated_wait(&apply) {
            Ok(Some(0)) => {
                let mut steps =
                    vec!["Installed Flux (administrator)".to_string()];
                if opts.launch_after {
                    match engine::launch(opts.scope) {
                        Ok(()) => steps.push("Launched Flux".into()),
                        Err(e) => {
                            return Outcome {
                                ok: true,
                                steps,
                                error: Some(e.to_string()),
                            }
                        }
                    }
                }
                Outcome { ok: true, steps, error: None }
            }
            Ok(Some(code)) => Outcome {
                ok: false,
                steps: vec![],
                error: Some(format!("The installer exited with code {code}.")),
            },
            Ok(None) => Outcome {
                ok: false,
                steps: vec![],
                error: Some("Administrator approval was declined.".into()),
            },
            Err(e) => Outcome {
                ok: false,
                steps: vec![],
                error: Some(e.to_string()),
            },
        }
    }
}
