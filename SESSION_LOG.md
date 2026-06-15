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

### Known Issues / TODO
- (to be filled as found)
