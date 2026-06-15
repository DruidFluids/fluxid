# Fluxid (fluidmonitor-rs) — Session Log

## Session: 2026-06-15 (overnight autonomous polish)

### Starting State
Contrary to the overnight brief's assumption of a barebones/stub project, the
repo is a **mature, feature-complete product**:

- `cargo build` — **clean** (0 errors).
- `cargo clippy --workspace` — **clean** (0 warnings).
- `target\debug\fluxid.exe` — **launches**, borderless always-on-top window
  ("Fluxid Widget"), no panics on stderr. (Binary is `fluxid`, not
  `fluidmonitor`.)

Everything in the brief's Phases 1–7 already exists and is polished:

- **Tiles**: CPU/GPU/RAM/Network/Disk/Clock, live sysinfo data, monospace-ish
  layout, warn/flash, per-tile show/hide + field toggles, custom names.
- **Themes**: 57 built-in presets + 25 bundled game-franchise packs (Theme
  Store with install/remove, per-game folders).
- **Skins**: 16 built-ins (Default/Minimal/Sharp/Glass/Carbon/Neon/Cyberpunk/…)
  with glow, gradient, sheen, accent/header bars, texture overlays; plus
  user-installable JSON skins (range-clamped, data-only).
- **Settings window**: tabbed, fully themed custom controls (sliders, togglers,
  pick_lists, dark inputs), live hot-apply.
- **Tray**: icon + Settings/Show/Game Mode/Exit menu.
- **Game mode**, **warnings system** (flash + temp gradient), **global
  hotkeys**, **edge + window snapping**, **opacity**, **UI scale**,
  **run-at-startup**, **click-through**, **updates checker**, **remote
  monitoring** (fluid-remote crate, popouts per device), **optional PawnIO CPU
  temp driver** install flow.

Crates: fluid-core, fluid-sensor, fluid-ipc, fluid-remote, fluid-widget
(main bin `fluxid`), fluid-service, fluid-setup. ~8,000 lines of Rust.

iced 0.13, sysinfo 0.34, serde/serde_json, tray-icon, windows crate.

### Approach
Because the product is already polished and is a GUI widget that can't be fully
visually diffed through automation, this session targets **concrete, low-risk
correctness + polish improvements** verified by reasoning and `cargo build` /
`clippy`, not sweeping rewrites. Each change compiles before commit.

### Completed
- [x] Verified build / clippy / launch (gate zero passed on arrival).
- [x] **settings.rs**: atomic save (temp-file + rename) so a kill mid-write can't
  truncate `settings.json`; on a corrupt/unparseable config, back it up to
  `settings.json.bak` before resetting to defaults (was silently destroying the
  user's settings on the next save). Addresses Phase-6 "missing/corrupt config".
- [x] **fmt.rs**: NaN/Inf guard in `fmt_net`/`fmt_disk` (a NaN rate fell through
  every `<` comparison to the GB/s arm and printed "NaN").
- [x] **style.rs `parse_hex`**: malformed hex (non-hex digits) now falls back to
  the caller's default instead of collapsing to pure black (which could make
  theme text invisible); also guards non-ASCII input against a byte-slice panic.
- [x] **tile.rs**: `pct()` helper guards CPU/GPU/RAM percentage readouts against
  non-finite sensor values; `sub_header` clips long hardware/disk-model names to
  one line (`Wrapping::None`) instead of word-wrapping into the fixed-height tile.

All changes verified: `cargo clippy -p fluid-widget` clean, app relaunches OK.

### Completed (round 2 — found via main.rs / settings_panel.rs / popups.rs review agents)
- [x] **main.rs `ResetDefaults`**: was leaving the live `RemoteManager` polling
  the just-removed devices (every other device mutation calls `set_devices`, this
  one didn't). Now pushes the empty device list + disabled-feed state to the
  runtime, clears cached `remote_snapshots`/`remote_conn`, resets `widget_device`,
  and preserves the machine's handshake key (runtime identity, not a preference).
- [x] **main.rs `ImportAppearanceCode`**: an imported share-code can change the
  skin (different `tile_spacing` → different `widget_size`), but the handler never
  resized the window. Now calls `resize_widget()` on success like every other
  appearance handler.
- [x] **main.rs `build_device_from_form`**: returned `Option`, so a present-but-
  malformed handshake key showed the generic "Fill in all fields first". Now
  returns `Result<_, &str>` and surfaces "Invalid handshake key" distinctly
  (Test + Save device handlers updated).
- [x] **popups.rs `warn_card`**: the "dim when disabled" branch was a no-op
  (`container(body).style(default)` renders identically). Now genuinely fades the
  card's text/muted/accent (alpha ×0.4) when the alert is off, a real inactive cue.

All verified: `cargo clippy -p fluid-widget` clean, app relaunches OK.

### Reviewed and confirmed NOT bugs (by the review agents — recorded so they
### aren't re-investigated): opacity %↔0..1 conversions, all per-tile field
### toggles' read/write wiring, font-offset + spacing slider message mapping,
### `SetInterval` f32→u64 round-trip, modulo-by-zero in theme/skin/traffic
### cycling, `warn_mut().unwrap()`, `ignore_next_move` handling, preset
### slot-bounds, subscription timer floors, empty-list guards in settings/popups.

### Completed (round 3 — interactive, screenshot-verified with the user)
Screenshots captured via `PrintWindow(PW_RENDERFULLCONTENT)` (GDI CopyFromScreen
returns black for iced's GPU/DComp surface) + a per-monitor-DPI-aware capture
process (widget lives on a 150%-scaled monitor).
- [x] **Network/Disk numbers centered on the tile centerline** (`centered_stat_line`
  in tile.rs): number flanked by two equal `Length::Fill` cells so it stays dead-
  centre and grows symmetrically as digit count changes — no measuring needed.
  Replaces the abandoned fixed-width-cell attempt (which couldn't fit the tile).
- [x] **R:/W: labels + traffic arrows pinned to the left edge** (left cell
  left-aligned) so they never move; only the centered number/unit shift.
- [x] **Long Disk/Network names truncate with “…”** (`fit_name`) sized to the tile
  width, so "Model · C:" / long adapter names cut off cleanly (no wrap/clip).
- [x] **Retro skin top bar** slimmed from `header_bar: 20.0` (≈30px @150%) to `6.0`.
- [x] **Randomize now includes installed Theme Store themes** in the colour pool
  (uninstalling removes them from `installed_themes`, so they leave the pool too).
- [x] **Live CPU clock (MHz)**: was sysinfo's static base freq; now PDH
  `% Processor Performance × base` on Windows (matches Task Manager, shows boost).
  Verified on-machine: 4698 MHz idle → 5045 MHz under load.
- [x] **Network/Disk position sliders**: the old "spacing" sliders didn't fit the
  centered layout. Reworked `centered_stat_line` to take a `left_inset` (clamped to
  tile width) — the slider now slides R:/W: (and the ↓/↑ arrow) left/right while
  the number stays centred and the unit hugs it with a small fixed gap. Renamed to
  "R: / W: position" and "Arrow position".
- [x] **Reclaimed 13.68 GB** by deleting `target/debug/incremental` (compiler
  cache that ballooned from the session's rebuilds). Project 26.65 GB → 12.96 GB.
  Root cause of the "massive folder": `target/` build artifacts (gitignored), not
  the source.

### Completed (round 4 — position-slider safety guards, empirically tuned)
- [x] **Dynamic clamp on the label/arrow inset** (`centered_stat_line`): computed
  the safe inset from real geometry — each Fill side cell is
  `(tile_inner − 2·gap − widest_4digit_number)/2`; the inset is clamped so
  `inset + label_w` always fits inside that half. Past it, iced grows the Fill to
  fit, shoving the centred number off-centre and clipping the unit. Verified by a
  forced-worst-case (`8888 MB/s`) inset sweep: broke at ~24px pre-clamp; with the
  clamp, a stored inset of 40 renders perfectly centred. Holds for any tile
  size / UI scale / font / digit count.
- [x] **Slider maxes set to the safe limit**: Network "Arrow position" 0–8
  (tighter — the arrow box is wider than R:/W:, so the centred 4-digit number
  leaves it less room), Disk "R: / W: position" 0–14. Number stays perfectly
  centred across the whole usable range.

### Findings deferred (real but higher-risk — left for a visually-verified pass)
- Width jitter when a byte-rate or VRAM value crosses the 10.0 boundary
  (`"9.9"`→`"10"` changes char count, shifting the content-sized value cell).
  ATTEMPTED a fixed-width right-aligned number cell, but reverted it: the tile's
  inner width (~110px at default size) only fits the worst case because the
  4-glyph value ("1023") only ever pairs with the narrowest unit ("B/s"); a fixed
  cell sized for 4 glyphs overflows/clips when paired with "MB/s". A correct fix
  needs coordinated spacing/width changes verified on-screen, so it's deferred
  rather than risk a clipping regression. (Comment left in `line_value`.)

### Decisions Made
- DECISION: Treat this as a polish/bug-hunt session on a mature codebase rather
  than a from-scratch build, since Phases 1–7 already ship. Keep changes small,
  reviewable, and individually committed.

## Session: 2026-06-15 (custom installer)

Built out `fluid-setup` from a do-nothing stub wizard into a working
self-contained custom installer. Key realisation: the Rust payload is a **single
embedded-asset exe** — `fluxid.exe` statically bundles all 25 themes, fonts,
icon, and PawnIO `.bin` modules via `include_bytes!`, the widget polls sensors
in-process (no Windows service), and there's no .NET runtime. So unlike the C#
`Fluid.Setup` (service + .NET check + admin), the installer's whole job is: copy
one exe, make shortcuts, register the uninstaller, apply opt-ins, launch.

### Architecture
- **One exe, three modes** (CLI dispatch in `main.rs`): no args → iced wizard;
  `--apply <flags>` → headless install engine (also the elevated worker);
  `--uninstall <flags>` → headless uninstall engine (registered as the ARP
  uninstall command, a copy of setup placed at `<dir>\uninstall.exe`).
- **Payload embed** (`build.rs` + `payload.rs`): reads `FLUXID_PAYLOAD` env var
  → copies `fluxid.exe` to `OUT_DIR/payload.bin` for `include_bytes!`; unset →
  0-byte placeholder so plain `cargo build --workspace` stays green (installer
  detects empty payload and refuses to install — "dev build").
- **Per-user vs all-users** (`engine.rs::Scope`): per-user → `%LOCALAPPDATA%\
  Fluxid`, HKCU, no UAC; all-users → `%ProgramFiles%\Fluxid`, HKLM, needs
  elevation → the unelevated GUI relaunches itself `--apply` with the `runas`
  verb (`ShellExecuteExW`) and waits, then launches the widget unelevated.
- **Operations** (`engine.rs`): copy exe, copy self→uninstall.exe, Start Menu
  shortcut (always) + Desktop (opt) via COM `IShellLinkW`/`IPersistFile`, ARP
  registry entry (DisplayName/Version/Publisher/DisplayIcon/InstallLocation/
  Uninstall+QuietUninstallString/EstimatedSize/NoModify/NoRepair), HKCU `Run`
  for startup, launch-on-finish. Uninstall reverses all of it (taskkill /F
  fluxid first, like the C#; optional `%APPDATA%\Fluxid` settings wipe; defers
  install-dir removal — which holds the running uninstaller — to a detached
  `cmd /C ping … & rmdir`).
- **Deferred to in-app (intentional, no divergent flows):** PawnIO driver (has a
  secure verified opt-in in `cpu_driver.rs`) and the remote firewall rule.

### Packaging
- `scripts/Build-Setup.ps1`: release-builds fluxid → sets `FLUXID_PAYLOAD` →
  release-builds fluid-setup (embeds) → copies to `dist\fluidmonitor-setup-v<ver>
  .exe` + writes a `.sha256` sidecar (release flow publishes checksums). `dist/`
  gitignored. Output verified: single 20.3 MB self-contained installer.

### Gotchas hit & fixed
- `SHELLEXECUTEINFOW` needs the `Win32_System_Registry` windows-crate feature
  (it carries an HKEY field).
- `BOOL` lives at `windows::core::BOOL` in 0.62; but `IPersistFile::Save` takes a
  plain `bool` now.
- `SHGetKnownFolderPath` `htoken` param is `Option<HANDLE>` → pass `None`.
- COM must be initialised on the thread before `CoCreateInstance(ShellLink)` —
  `CoInitializeEx`/`CoUninitialize` around shortcut creation (missing it failed
  the install right after the file copy).
- Self-delete `rmdir` must be passed to `cmd.exe` via `raw_arg` — `Command::arg`
  backslash-escapes the path quotes, which `cmd` doesn't grok, so it no-ops.

### Verified on-machine
- Headless per-user `--apply --desktop --startup` → exit 0; both exes, both
  shortcuts, HKCU Run, full ARP entry all present and correct (EstimatedSize
  ≈34 MB).
- Silent uninstall → shortcuts/Run/ARP removed, install dir gone after the
  deferred rmdir. Zero trace left.
- GUI wizard launches without crashing.

### Needs manual user test (couldn't automate)
- The **all-users** path (real UAC elevation prompt) — same engine code, just
  HKLM + Program Files via the elevated `--apply` worker.
- Full click-through of the GUI wizard and the ARP "Uninstall" button from
  Settings → Apps.

### Round 2 — full CLI switch coverage, widget theme match, docs
- **Every installer feature now has a documented CLI switch** (`cli` module in
  main.rs, one source of truth). Flags accept `--flag` / `-flag` / `/flag`
  case-insensitively. Modes: `--install`/`--apply`, `--uninstall`,
  `/S`|`/q`|`--silent`|`--quiet` (silent; alone = headless install with
  defaults), `--help`/`/?`. Install opts: `--scope per-user|all-users`,
  `--no-desktop`, `--no-startup`, `--no-launch`, `--all`. Uninstall opts:
  `--scope`, `--remove-settings`, `--silent`. Headless install now **defaults to
  everything on**; the GUI→elevated worker call passes explicit `--no-*` +
  always `--no-launch` so nothing is ambiguous and the worker never launches the
  widget elevated. Built-in `--help` text (console in debug, MessageBox in the
  windowed release).
- **Installer themed to match the widget's "Dark (default)"** (`style.rs`):
  bg #1E1E22, tile #2A2A30, accent #00A8FF, text #E8E8EC, muted #9A9AA8. Custom
  iced `Theme` palette + container/card/title/muted/button styles; options on a
  rounded tile-colored card; accent headings; primary(accent)/secondary buttons.
  Window/taskbar **icon** = the widget's logo PNG (copied to
  `crates/fluid-setup/assets/icon.png`, decoded via the `image` crate, set with
  `iced::window::icon::from_rgba`).
- **Docs:** top-level `README.md` (intro + Install/silent/uninstall + build +
  workspace table) and `docs/INSTALLER.md` (quick start, scope table, full
  switch reference with examples, what-gets-created layout, uninstall, build
  steps, architecture, signing/SmartScreen). Fixed ARP `URLInfoAbout` to the
  real remote `DruidFluids/fluidmonitor-rs`.
- Verified: `--apply --no-desktop --no-launch` → start-menu yes, desktop
  skipped, startup on, fluxid not launched; silent uninstall full cleanup;
  `--help` + `/?` print correctly; themed GUI launches without crashing.

### Round 3 — redesigned wizard to a centered, modern layout (screenshot-verified)
Reworked the wizard look to match a reference the user liked (the old C# setup):
step indicator on top → centered content → divider → centered button pair.
- `style.rs`: accent **icon badge** drawn with canvas (accent circle + white ECG
  pulse), outlined `secondary`/accent `primary` buttons, `segment`/`divider`
  container styles, white `heading`, `accent_text`. Window 500×500, logo icon.
- `main.rs`: `frame(step, content, buttons, center)` assembles every page;
  `step_bar` (4 accent segments). Pages return `(content, buttons)`. Welcome =
  badge + "Fluxid" + version + MIT line; Options; Installing; Done = ✓ checklist.
- Hidden `--page welcome|options|installing|done` to open the wizard on a page
  (QA/screenshots); Done injects a sample outcome.
- **Captured all 4 pages via DPI-aware PrintWindow(PW_RENDERFULLCONTENT)** and
  fixed two bugs found:
  1. Options page clipped the 3rd checkbox — content was vertically centered but
     taller than the area. Fix: top-align the Options page (others stay
     centered) + taller window.
  2. Done ✓ rendered as tofu — iced's default font lacks U+2713. Fix: load
     `C:\Windows\Fonts\seguisym.ttf` (like the widget) and render the check in
     `Segoe UI Symbol` + accent color, label in default font.
  Re-captured: all four pages correct.

### Known Issues / TODO
- (to be filled as found)
