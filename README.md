<div align="center">

<img src="docs/images/icon.png" alt="Flux" width="96" height="96">

# Flux

**A beautiful, lightweight system monitor widget for Windows.**

Real-time CPU, GPU, RAM, network, and disk stats — always on your desktop, never in your way.

[![Release](https://img.shields.io/badge/release-v1.0.6-5898a0)](../../releases)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4)](#requirements)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20iced-dea584)](https://iced.rs)
[![License](https://img.shields.io/badge/license-Personal%20Use-c0392b)](LICENSE)

<br>

<img src="docs/images/hero.png" alt="Flux widget" width="260">

</div>

---

## Why Flux?

Most system monitors are either heavyweight dashboards or cryptic taskbar numbers. Flux sits in between — a clean, themeable widget that shows exactly what you care about at a glance, with virtually zero overhead.

- **One tiny executable** — no background service, no .NET runtime, no browser engine. The widget polls hardware in-process and renders on the GPU.
- **Beautiful by default, yours in two clicks** — a library of skins, 100+ color presets, full font control, or roll the dice and let it surprise you.
- **Built for gamers** — Game Mode snaps the widget to a corner with one hotkey, even in fullscreen.
- **Remote monitoring** — watch your other PCs' stats from one desktop over your LAN.
- **Rust, for reach** — a from-scratch rewrite of the original C# app, built for broad hardware coverage and a path to Linux/macOS.

---

## Features

### Live hardware tiles

CPU, GPU, RAM, Network, Disk, and Clock tiles — each individually toggleable and **drag-to-reorder**. CPU and GPU tiles show temperature, load, and clock speed. RAM shows usage and capacity. Network shows live up/down traffic with animated indicators. Disk shows real-time read/write speeds.

Vertical or horizontal layout — switch any time.

<div align="center">
<img src="docs/images/widget-horizontal.png" alt="Horizontal layout">
</div>

### Themes, skins, and colors

The appearance engine has three independent layers:

| Layer | What it controls | Count |
|-------|-----------------|-------|
| **Skins** | Shape, borders, tile style, corner radius | 16 built-in |
| **Colors** | 5-color palette (background, tile, accent, text, muted) | 100+ presets |
| **Preset Themes** | One-click skin + color combos | Curated library + downloadable packs |

Hit the dice for a random look, undo if you don't like it, and save your favorites to 5 quick slots. Import and export themes as share codes to swap with others, and browse downloadable theme packs from the built-in Theme Store.

<div align="center">
<img src="docs/images/settings-appearance.png" alt="Appearance settings" width="420">
</div>

### CPU temperature

A one-time sensor driver setup ([PawnIO](https://pawnio.eu/)) unlocks CPU temperature directly on the widget. The driver is downloaded on demand from the official source — **Flux never bundles or redistributes it**. Switch between °C and °F with a rocker, and remove the driver any time from the same menu.

### Game Mode

Press a hotkey and the widget snaps to a corner of your screen with custom opacity, layout, and tile selection — designed to stay readable but unobtrusive over a game. Press again to send it back. Works in fullscreen.

<div align="center">
<img src="docs/images/game-mode.png" alt="Game Mode settings" width="420">
</div>

### Temperature warnings

Set a threshold and the widget flashes a warning color when your CPU or GPU runs hot. Or use gradient mode, where the unit text shifts smoothly from cool blue to hot red as temperature climbs.

<div align="center">
<img src="docs/images/warnings.png" alt="Warnings settings" width="420">
</div>

### Utilities

Quick launchers for popular Windows optimization tools and a window-snap blocklist with a live window picker.

<div align="center">
<img src="docs/images/utilities.png" alt="Utilities window" width="420">
</div>

### Remote monitoring

Run Flux on multiple machines and watch them all from one desktop. TCP over TLS with mutual handshake-key authentication. Each remote device gets its own popout widget with independent layout and theming.

<div align="center">
<img src="docs/images/remote-monitoring.png" alt="Remote monitoring" width="420">
</div>

### Quality of life

- **Snap to edges and windows** — the widget docks flush to screen edges and other windows' borders, with a configurable blocklist
- **Click-through mode** — make the widget invisible to the mouse; toggle back with a hotkey
- **Slider default markers** — every settings slider shows a tick at its factory default that glows as you approach it
- **Built-in help** — the **?** button opens a categorized guide to every feature
- **Dark and light mode** — full palette swap with one click
- **Run at startup** — per-user, no admin needed
- **Crash-hardened** — automatic render recovery and crash logging

---

## Security

Flux is built with security-conscious defaults:

- **No telemetry** — the app makes zero analytics calls. The only outbound connections are the optional update check, the optional PawnIO driver download (user-initiated), and LAN-only remote monitoring.
- **PawnIO is never bundled** — the CPU temperature driver is downloaded on demand from its [official GitHub release](https://github.com/namazso/PawnIO.Setup/releases), and is never redistributed here.
- **Verified updates** — the in-app updater refuses to run a downloaded installer unless its SHA-256 matches a checksum published alongside the release.
- **Scanned on VirusTotal** — every release is scanned and the result is linked in its notes. v1.0.6: **[0 / 69](https://www.virustotal.com/gui/file/a50d50defe82c78ea8687bfa85e5577ca1c73bf1e1e125de0b5b4ee1e626dfb2)** (clean).
- **Unsigned build** — the installer is not code-signed, so Windows SmartScreen shows a one-time prompt. Verify any download against the `.sha256` published with each release before running it.
- **Settings stay local** — all configuration lives in `%APPDATA%\Flux`. Nothing is sent anywhere.
- **Source-available** — every line is in this repo for inspection (see [License](#license)).

---

## Installation

1. Download the latest **`flux-setup-vX.Y.Z.exe`** from [**Releases**](../../releases).
2. (Recommended) verify it against the published checksum:
   ```powershell
   Get-FileHash .\flux-setup-vX.Y.Z.exe -Algorithm SHA256
   ```
3. Run it. The build is unsigned, so SmartScreen shows a one-time prompt — click **More info → Run anyway**.
4. Follow the wizard: **Just me** (no admin) or **All users**, pick the optional desktop shortcut / startup / launch, and **Install**.

The installer is a small, self-contained custom installer that embeds the widget — no separate download, no service, no .NET. It can also run silently for scripted deployments:

```bat
flux-setup.exe /S            :: silent per-user install
flux-setup.exe --help        :: list every switch
```

See [`docs/INSTALLER.md`](docs/INSTALLER.md) for the full command-line reference, install locations, the registry/shortcut layout, and uninstall instructions.

### Requirements

- Windows 10 / 11 (x64)
- A GPU with Direct3D 12, Vulkan, or DX11 support (virtually all modern PCs)

### Uninstall

**Settings → Apps → Flux → Uninstall** (or Control Panel → Programs and Features). Your settings in `%APPDATA%\Flux` are kept unless you uninstall from the command line with `--remove-settings`.

---

## Architecture

Flux is a single executable — no background service, no runtime to install.

```
┌───────────────────────────────────────────┐       TLS (optional)        other
│  Flux  (flux-widget)                    │ ◄───────────────────────►   machines
│  • polls hardware in-process (flux-sensor)│    remote sensor sharing    running
│  • renders tiles on the GPU via iced/wgpu  │                             Flux
└───────────────────────────────────────────┘
```

| Crate | What it is |
|-------|------------|
| `flux-widget` | The widget app (binary `flux`). |
| `flux-sensor` | Hardware polling — sysinfo, NVML for NVIDIA, optional PawnIO for CPU temp. |
| `flux-core` | Shared settings and types. |
| `flux-remote` | Remote-monitoring transport (TLS). |
| `flux-setup` | The self-contained installer (binary `flux-setup`). |

---

## Building from source

Requires a recent stable Rust toolchain on Windows.

```powershell
git clone https://github.com/DruidFluids/Flux.git
cd Flux

# Run the widget directly
cargo run -p flux-widget --release

# Build the widget and the installer
cargo build -p flux-widget -p flux-setup --release
# -> target\release\flux.exe  and  target\release\flux-setup.exe
```

---

## License

**Personal Use License** — © 2026 Matt Hakes (DruidFluids). Source-available, **not** open-source.

You **may** download, build, run, and **modify** Flux for your own use. You **may not** redistribute it — publishing, sharing, mirroring, sublicensing, or selling the software or its source (original or modified, source or binary) requires prior written permission. See [`LICENSE`](LICENSE) for the exact terms.

---

<div align="center">
<sub>Built with Rust, iced, and an unreasonable number of color palettes.</sub>
</div>
