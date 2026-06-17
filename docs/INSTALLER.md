# Flux Installer

`flux-setup.exe` is a small, self-contained custom installer for the
Flux widget. It is written in Rust (the `flux-setup` crate) and **embeds the
widget inside itself** — there is no separate payload to download, no Windows
service to register, and no .NET or other runtime dependency. Its whole job is
to copy one executable, create shortcuts, register an uninstaller, apply the
opt-ins you choose, and (optionally) launch Flux.

- [Quick start](#quick-start)
- [Where Flux installs](#where-Flux-installs)
- [Command-line switches](#command-line-switches)
- [What the installer creates](#what-the-installer-creates)
- [Uninstalling](#uninstalling)
- [Building the installer](#building-the-installer)
- [How it works](#how-it-works)
- [Code signing & SmartScreen](#code-signing--smartscreen)

## Quick start

1. Download `flux-setup-vX.Y.Z.exe` from the
   [Releases](https://github.com/DruidFluids/Flux/releases) page.
2. Run it. Windows SmartScreen may show a “Windows protected your PC” prompt
   because the build is unsigned — click **More info → Run anyway**
   (see [below](#code-signing--smartscreen)).
3. The wizard walks you through three steps: **Welcome → Setup options →
   Done**. Pick whether to install for just you or all users, tick the optional
   shortcuts/startup, and click **Install**.

The setup window uses Flux's own “Dark (default)” theme, so it looks like the
app it installs.

## Where Flux installs

You choose the scope in the wizard (or with `--scope`):

| Scope | Install folder | Registry | Admin (UAC)? |
|-------|----------------|----------|--------------|
| **Just me** (per-user, default) | `%LOCALAPPDATA%\Flux` | `HKCU` | No |
| **All users** | `%ProgramFiles%\Flux` | `HKLM` | Yes — one prompt |

Per-user is the default and needs no administrator rights. All-users installs
for everyone on the machine and triggers a single Windows administrator prompt;
the installer relaunches itself elevated only for the file/registry work.

The startup opt-in (“Start Flux with Windows”) is always written to the
current user’s `HKCU\…\Run`, regardless of scope.

## Command-line switches

Every feature the wizard offers also has a switch, so the installer can be
scripted or run silently. **Each switch accepts `--flag`, `-flag` or `/flag`,
case-insensitive** (so `--silent`, `-silent`, `/silent` and `/S` are the same).

### Modes

| Switch | Meaning |
|--------|---------|
| *(no switches)* | Launch the graphical setup wizard. |
| `--install`, `--apply` | Install without the wizard (headless). |
| `--uninstall` | Uninstall. This is exactly what Add/Remove Programs runs. |
| `/S`, `/q`, `--silent`, `--quiet` | Silent: no wizard and no message boxes. On its own (no other mode), this performs a headless install with the default options. |
| `--help`, `/?` | Show the built-in switch reference. |

### Install options

Headless installs default to **installing everything** (desktop shortcut +
startup + launch) for the current user. Opt out per-feature:

| Switch | Meaning |
|--------|---------|
| `--scope per-user` | Install for the current user (no admin). **Default.** |
| `--scope all-users` | Install for all users (prompts for administrator). |
| `--no-desktop` | Do not create a desktop shortcut. |
| `--no-startup` | Do not start Flux with Windows. |
| `--no-launch` | Do not launch Flux when setup finishes. |
| `--all` | Explicitly enable every optional feature (this is the default). |

### Uninstall options

| Switch | Meaning |
|--------|---------|
| `--scope <per-user\|all-users>` | Match the scope Flux was installed with. |
| `--remove-settings` | Also delete `%APPDATA%\Flux` (settings, themes, skins). |
| `/S`, `--silent` | Uninstall with no completion/error message box. |

### Examples

```bat
:: Silent per-user install of everything (scripted deployment)
flux-setup.exe /S

:: Headless install, no desktop icon, don't auto-launch
flux-setup.exe --install --no-desktop --no-launch

:: All-users install (will prompt for admin), no startup entry
flux-setup.exe --install --scope all-users --no-startup

:: Silent uninstall that also wipes user settings
flux-setup.exe --uninstall --scope per-user --silent --remove-settings
```

## What the installer creates

For a per-user install (all-users uses the all-users folders and `HKLM`):

**Files** — in `%LOCALAPPDATA%\Flux\`:
- `flux.exe` — the widget.
- `uninstall.exe` — a copy of the installer; this is what runs on uninstall.

**Shortcuts**:
- Start Menu: `…\Programs\Flux.lnk` (always).
- Desktop: `Flux.lnk` (unless `--no-desktop`).

**Registry**:
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Flux` — the
  Add/Remove Programs entry (`DisplayName`, `DisplayVersion`, `Publisher`,
  `DisplayIcon`, `InstallLocation`, `UninstallString`, `QuietUninstallString`,
  `EstimatedSize`, `NoModify`, `NoRepair`).
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Flux` — only if startup
  is enabled.

## Uninstalling

- **Settings → Apps → Installed apps → Flux → Uninstall**, or **Control Panel
  → Programs and Features**. Either runs `uninstall.exe --uninstall`.
- Or from the install folder: `uninstall.exe --uninstall --scope per-user`
  (add `--silent` and/or `--remove-settings` as needed).

Uninstall force-closes a running Flux first, removes the shortcuts, the
startup entry and the Add/Remove Programs entry, then deletes the install
folder. Your settings in `%APPDATA%\Flux` are **kept** unless you pass
`--remove-settings`.

## Building the installer

The installer embeds `flux.exe` at build time, so it is built in two steps.
The provided script does both:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\Build-Setup.ps1
```

This will:
1. Release-build the widget (`cargo build -p flux-widget --release`).
2. Set `FLUX_PAYLOAD` to the built `flux.exe` and release-build
   `flux-setup`, whose `build.rs` embeds the exe via `include_bytes!`.
3. Copy the result to `dist\flux-setup-v<version>.exe` and write a
   `.sha256` checksum next to it.

> A plain `cargo build` of the workspace (without `FLUX_PAYLOAD`) still
> compiles `flux-setup`, but it embeds an **empty** payload — that build is a
> dev build and refuses to install, telling you to use the script.

## How it works

The installer is **one executable with three modes**, selected by CLI args:

- **no args →** the iced wizard GUI.
- **`--apply` →** the headless install engine. The wizard also uses this as its
  *elevated worker*: for an all-users install the unelevated GUI relaunches
  itself with the `runas` verb and these flags, waits for it, then launches the
  widget unelevated.
- **`--uninstall` →** the headless uninstall engine. The installer copies itself
  to `uninstall.exe` in the install folder and registers that as the
  Add/Remove Programs uninstall command.

Because the install folder holds the running `uninstall.exe`, the uninstaller
deletes the widget and registry entries immediately and hands the final
directory removal to a short detached `cmd` step that runs once the uninstaller
exits.

CPU-temperature sensing (the optional PawnIO driver) and the remote-monitoring
firewall rule are **not** handled by the installer — they have their own
explicit, security-gated opt-ins inside Flux’s settings.

## Code signing & SmartScreen

Flux is currently shipped **unsigned** (there is no code-signing budget yet),
so the first run shows a one-time SmartScreen “Run anyway” prompt. Every release
publishes a **SHA-256 checksum** so you can verify the download:

```powershell
Get-FileHash .\flux-setup-vX.Y.Z.exe -Algorithm SHA256
```

Compare the result against the `.sha256` file from the release. The build is
wired to be sign-ready, so Authenticode signing can be enabled later without
rework.
