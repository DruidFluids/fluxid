# fluxid-rs — Overnight Autonomous Polish Session

## Project Context

fluxid is a Rust rewrite of fluxid, a Windows system monitoring desktop widget inspired by NZXT CAM's mini-mode. The project lives at `C:\dev\fluxid`. It uses a Cargo workspace with crates including:

- `fluid-core` — shared types, settings (serde JSON), theme/color definitions
- `fluid-ipc` — named pipe communication (interprocess crate)
- `fluid-sensor` — hardware metric polling via `sysinfo`
- `fluid-widget` — iced 0.13 UI, main binary (`fluxid`)
- Possibly `fluid-daemon` or `fluid-service` — background sensor service

The original WPF/.NET 8 version (at `C:\dev\fluid\fluid`) is the reference implementation with 140+ color themes, 5 skins, tile-based layout (CPU/GPU/RAM/Network/Disk), a settings window, game mode, warnings system, and extensive customization. The Rust version is a rewrite targeting feature parity and beyond. **The WPF version is the gold standard — when in doubt about how something should look or work, that's your reference.**

## First Step: Understand Current State

Before doing ANYTHING else:

1. `cargo build` — does it compile? If not, fix ALL compile errors first. This is gate zero.
2. `cargo run --bin fluxid` — does it launch? What renders? Screenshot yourself mentally.
3. Read through every `.rs` file in every crate. Understand what exists, what's stubbed, what's broken.
4. Read `Cargo.toml` (workspace root and each crate) to know exact dependency versions.
5. Log your findings in `SESSION_LOG.md` under "Starting State".

**Do not start building until you have a compiling, launching application.** If it doesn't compile, that's your first job. If it compiles but crashes on launch, that's your second job. Get to a running window before anything else.

## Your Mission

You are working **fully autonomously** with zero human interaction. No questions, no stopping, no waiting. Your goal is to take fluxid from its current state and make it as polished, complete, and visually refined as possible. Every pixel, every interaction, every transition should feel intentional and crafted.

This is a **polish and completeness** session. The WPF version has years of iteration behind it. Your job is to close the gap as much as possible in one long session.

## Operating Rules

### Decision-Making
- **Do not stop to ask questions.** If something is ambiguous, make your best judgment call, leave a `// DECISION: <rationale>` comment in the code, and log it in `SESSION_LOG.md`.
- **Compile after every major change.** `cargo build` frequently. Never leave the project broken.
- **Commit often.** `git add -A && git commit -m "descriptive message"` after each working milestone.
- **Don't spend more than ~10 minutes stuck on any single problem.** Simplify and move on. Leave a `// SIMPLIFY:` comment.
- **Always check iced 0.13 API docs/examples if unsure about a widget API.** Don't guess at function signatures — iced's API changes between versions. If something doesn't compile, check the actual types.

### Priority Order

Work through these in order. Each phase should leave the app in a better, compiling, runnable state.

**Phase 1 — Get it running and rendering (if not already)**
- Fix any/all compile errors
- Get a window on screen showing real hardware data (CPU temp/load, GPU temp/load, RAM usage, Disk R/W, Network up/down)
- Tiles should render with live-updating numbers from sysinfo
- Basic window: borderless, always-on-top, draggable, dark background
- If any of this already works, skip ahead

**Phase 2 — Widget visual polish (the main widget people see on their desktop)**
- **Tile layout**: Each metric (CPU, GPU, RAM, Network, Disk) gets a tile card with a subtle background, rounded corners, consistent padding
- **Typography**: Clean font hierarchy — metric label (small, muted), value (large, bold), unit suffix (small, muted). Monospaced numbers so they don't jitter as values change.
- **Color theming**: Implement at minimum: background color, tile background color, text color, muted text color, accent color. Load from settings.json. Default dark theme should look sleek — dark gray background (#1a1a1a-ish), slightly lighter tile backgrounds (#252525-ish), white text, accent color for highlights.
- **Spacing**: Consistent gaps between tiles. Padding inside tiles. Nothing should feel cramped or awkwardly spaced. Aim for 8px grid alignment.
- **Orientation**: Support horizontal (tiles side by side) and vertical (tiles stacked) layout. Read from settings.
- **Opacity**: Window opacity controlled by settings value. Render with transparency.
- **Edge snapping**: Snap to screen edges when dragging near them.
- **Hover effects**: Subtle brightness/opacity change on tile hover.
- **Smooth number transitions**: Values should update smoothly, not jump. If iced supports it, animate. If not, at minimum update at a steady cadence (1-2 Hz) so it doesn't feel jerky.
- **Hardware names**: Show CPU name and GPU name somewhere (either as tile subtitles or in a tooltip).

**Phase 3 — Settings window**
- Separate window (or panel) that opens from tray icon or hotkey
- **Sections to implement** (mirror the WPF version's structure):
  - **Layout**: Orientation toggle (Horizontal/Vertical), opacity slider, UI scale slider, edge snapping toggle
  - **Appearance / Colors**: Theme selector/cycler. Implement at least 10-15 built-in color themes from the WPF version's catalog. Each theme = a set of hex colors (background, tile, text, muted, accent, border). Arrow cycler UI: `‹ Theme Name ›`
  - **Tiles**: Show/hide toggles for each tile (CPU, GPU, RAM, Network, Disk). Device selector dropdowns (which disk, which network adapter).
  - **General**: Start with Windows toggle, click-through mode toggle
- **Settings persistence**: Load/save to `%APPDATA%\fluxid\settings.json` via serde
- **Live preview**: Changes in settings should reflect on the widget immediately (hot-apply), not require save+restart
- **Visual style**: The settings window itself should be themed — dark background matching the widget, styled controls (not OS-native gray). Sliders, toggles, dropdowns should all be custom-drawn to match the aesthetic.
- **Bottom bar**: Save and Close (primary accent button), Reset to Defaults (danger/red button), styled to match — not default OS buttons.

**Phase 4 — System tray integration**
- Tray icon that persists when widget is running
- Right-click menu: Settings, Exit (minimal — not cluttered)
- Clicking tray icon toggles widget visibility
- Tray icon should be a simple custom icon (draw one programmatically or embed a small PNG)

**Phase 5 — Skins system**
- Surface-level skins: corner radius, border width, shadow, spacing, font overrides
- At least 3 built-in skins: Default (rounded, modern), Minimal (no borders, floating text), Sharp (square corners, dense, no shadows)
- Skin cycler in Settings: `‹ Skin Name ›` just like themes
- Hot-swap (apply immediately when cycling)

**Phase 5.5 — Pre-rendered high-res skins (attempt, skip if infeasible)**

This is an ambitious stretch goal: generate photorealistic, pre-rendered skin textures programmatically — think brushed aluminum bezels, carbon fiber tile backgrounds, frosted glass panels, wood grain frames, matte rubber borders. Not flat color rectangles — actual material textures that look like they were rendered in Blender.

**How to attempt this:**
- Use the `image` crate to generate high-res PNG textures (256x256 or 512x512) at startup or build time via a build script
- Layer procedural noise (Perlin, Simplex, Worley) to create material textures:
  - **Brushed Metal**: Horizontal streaks via directional noise, slight color gradient, specular highlight band
  - **Carbon Fiber**: Repeating diagonal weave pattern with subtle depth/shadow
  - **Frosted Glass**: Gaussian blur effect over subtle noise, slight transparency, diffused edges
  - **Dark Leather**: Low-frequency bumpy noise with fine grain overlay, warm dark browns
  - **Matte Black**: Very subtle noise over near-black, soft vignette at edges
  - **Walnut Wood**: Horizontal grain lines via stretched noise layers, warm browns with darker streaks
- Use these as tile backgrounds, window borders/bezels, and panel frames via iced's `Image` widget
- Each material skin = a set of generated textures (tile_bg, window_frame, panel_bg, separator) + matching color palette
- Generate at multiple resolutions if needed for different UI scale settings

**Target skins:**
- **Obsidian** — matte black with subtle noise, chrome/silver accent edges
- **Brushed Steel** — metallic gray with horizontal grain, blue-tinted highlights
- **Carbon** — carbon fiber weave, red accent stitching line details
- **Mahogany** — dark wood grain, brass/gold accents
- **Frosted** — translucent frosted glass over a blurred backdrop, minimal borders

**If this is too complex or the results don't look good:** Skip it entirely. Don't ship ugly procedural textures — flat themed colors done well are better than bad fake materials. Leave a `// TODO: pre-rendered material skins` comment and a note in SESSION_LOG.md about what you tried and why it didn't work. Move on to Phase 6.

**Phase 6 — Deep polish pass (iterate on everything)**

Go back to the beginning and make everything better. This is the most important phase. For each element, ask: "Would this look good in a product screenshot? Would a user think this was made by a professional?"

- **Widget**:
  - Are the tiles perfectly aligned? Check padding, margins, spacing pixel by pixel.
  - Do the numbers render cleanly? No jitter, no clipping, no overflow.
  - Does the window feel solid? No flicker on resize/move, smooth drag, proper layering.
  - Is the opacity/transparency working correctly with the compositor?
  - Do the tiles have any visual hierarchy? CPU/GPU should feel like "primary" tiles, Network/Disk more secondary.
  - Add subtle separators or dividers between tiles if it helps readability.
  - Ensure text contrast meets readability standards against all theme backgrounds.

- **Settings**:
  - Are all controls aligned in clean columns?
  - Do sliders feel responsive? Do they show their current value?
  - Do dropdowns open/close cleanly?
  - Is there enough spacing between sections? Clear section headers?
  - Does tab/keyboard navigation work?
  - Does the window size itself correctly (no scrollbar unless needed, no wasted space)?

- **Themes**:
  - Do all themes look good? No white-on-white or dark-on-dark readability issues?
  - Does the muted text color actually look muted (lower contrast) against the background?
  - Does the accent color pop appropriately?

- **Interactions**:
  - Hover states on everything interactive (buttons, tiles, tray icon)
  - Press/click feedback (visual depression or color shift)
  - Smooth transitions wherever iced supports them
  - Keyboard shortcuts: Ctrl+Q to quit, Ctrl+S to save settings, Escape to close settings
  - Right-click context on the widget itself (Settings, Game Mode, Exit)

- **Edge cases**:
  - What happens with very long hardware names? Truncate with ellipsis.
  - What happens at extreme opacity (10%, 90%)? Still usable?
  - What happens at different UI scales? Does layout hold?
  - What if a sensor returns no data? Show "N/A" or "--", not a crash.
  - What if settings.json is missing or corrupt? Graceful fallback to defaults.

**Phase 7 — Keep going (infinite loop)**

If you've polished everything above and there's still time:
- Add more themes (the WPF version has 140 — franchise themes from games, movies, etc. Add as many as you can. Each theme is just a struct of hex colors.)
- Add the warnings system (visual flash/gradient when CPU temp exceeds threshold)
- Add game mode (simplified overlay layout, different positioning)
- Add the muted text visibility slider
- Add tile customization (network arrow spacing, disk label spacing)
- Add font selection (pick from system fonts)
- Add the color strip/slider for custom color editing
- Better tray icon (maybe showing current CPU temp as text)

## Asset & Visual Guidelines

- **No external asset files.** Everything generated in code — colors as hex constants, icons drawn with iced's canvas/SVG primitives, or embedded as const byte arrays.
- **Target a premium desktop widget aesthetic.** Think Rainmeter premium skins, NZXT CAM, or Corsair iCUE overlay. Clean, minimal, information-dense but not cluttered.
- **8px grid**: All spacing, padding, margins should be multiples of 4 or 8 pixels.
- **Font sizes**: Metric values ~18-24px, labels ~11-13px, section headers ~14-16px. Adjust to taste but be consistent.
- **Colors**: Dark themes should have enough contrast between background layers (window bg → tile bg → text). Light themes should avoid pure white — use off-whites.

## Technical Constraints

- **iced 0.13** — check the actual API. `Padding::from()` takes specific types. `pick_list` has specific generic constraints. When something doesn't compile, read the error carefully and fix to the actual iced 0.13 API, don't guess.
- **sysinfo** crate for hardware metrics. Refresh on a timer (1-2 sec interval). Don't poll on every frame.
- **serde + serde_json** for settings. Derive Serialize/Deserialize on settings structs. Handle missing fields with `#[serde(default)]`.
- **tokio** if needed for async IPC. But prefer synchronous where possible to avoid complexity.
- Avoid `unsafe`. Use `.unwrap_or_default()`, `if let`, `?` operator over `.unwrap()`.
- If you need additional crates (`noise` for procedural textures, `image` for PNG generation, `rand` for RNG), add them to the appropriate `Cargo.toml` and move on.
- `cargo clippy` should be clean.

## Session Logging

Maintain `SESSION_LOG.md` at the repo root:

```markdown
## Session: [date]

### Starting State
- What compiled? What rendered? What was broken?

### Completed
- [x] What you finished

### Decisions Made
- DECISION: Chose X over Y because Z

### Known Issues
- BUG: Description
- TODO: Things for next session

### Visual Changes
- Before/after descriptions of UI improvements

### Next Priorities
- What should be tackled next
```

## What NOT to Do
- Don't rewrite the architecture or restructure crates unless something is fundamentally broken
- Don't add networking/remote monitoring features
- Don't write unit tests this session — focus on the product
- Don't try to implement the Windows Service (fluid-daemon) if it doesn't exist yet — just poll sensors directly from the widget process for now
- Don't fight iced's paradigms — work with the framework, not against it

## Success Criteria

A person should be able to:
1. Launch fluxid and see a beautiful, dark-themed widget showing live CPU, GPU, RAM, Network, and Disk stats
2. Drag the widget around the screen smoothly, have it snap to edges
3. Right-click or tray-click to open a polished Settings window
4. Cycle through multiple color themes and see the widget update live
5. Toggle between horizontal and vertical layout
6. Adjust opacity and see it change in real time
7. Close settings, and the widget keeps running with the chosen look
8. Look at it and think "this looks professional, not like a hobby project"

That last one is the real bar. Make it beautiful.

## Start

`cargo build`, assess the current state, log it, then start working from Phase 1 down. Go.
