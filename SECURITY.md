# Flux — Security Posture

Primary goal: **zero security issues.** This document is the threat model and
the inventory of every privileged / network / code-execution surface, with the
protection on each. Keep it current when those surfaces change.

## Design principles
- **Local-first, no telemetry.** Settings live in `%APPDATA%\Flux`.
  Nothing is sent anywhere except (a) opt-in sensor snapshots to *authenticated*
  remote clients and (b) update checks to GitHub. No analytics, no tracking.
- **Least privilege.** The widget runs unelevated. The only operations that
  request elevation (UAC) are explicit, user-initiated, and one-time.
- **No download-and-execute of unverified code.** Anything fetched from the
  network is either verified (installer SHA-256) or treated as inert data, or
  simply opened in the browser for the user to run themselves.

## Network surfaces

| Surface | Direction | Transport | Authentication / verification |
|---------|-----------|-----------|-------------------------------|
| Update check | out → api.github.com | HTTPS | read-only JSON; TLS |
| Update download | out → GitHub asset | HTTPS | **mandatory SHA-256** before execution (see below) |
| Remote sensor feed (server) | in ← LAN, port 5199 | TLS 1.2/1.3 | self-signed cert, **HMAC-SHA256 challenge**, per-IP rate-limit (5 fails / 60 s) |
| Remote sensor feed (client) | out → user-configured host | TLS 1.2/1.3 | server cert **pinned by SHA-256**, HMAC handshake |

### Updater (`updates.rs`)
- Checks the latest GitHub release over HTTPS, compares semver.
- On download, the installer is written to `%TEMP%` and **only executed if its
  SHA-256 matches a checksum published in the release** (`<installer>.exe.sha256`
  or a `SHA256SUMS` asset). If no checksum is published, or it mismatches, the
  update is **refused** — not run. This is defence in depth on top of TLS: a
  compromised release/account cannot push an unverified binary that auto-runs.
- **Release requirement:** every release MUST ship a checksum asset, or
  auto-update silently no-ops by design.

### Remote feed (`flux-remote`)
- Off by default. Enabling it adds one Windows Firewall rule (TCP 5199, **private
  profile only**) — see below.
- Handshake key = `FM1:base64(certSHA256 ‖ hmacSecret)`. The client pins the
  server's exact cert fingerprint and proves knowledge of the HMAC secret; the
  server verifies the client's HMAC and rate-limits failures. Snapshots are
  sensor data only (no credentials, no PII).

## Privileged operations (each = explicit user action)

| Operation | When | Elevation | Notes |
|-----------|------|-----------|-------|
| Add firewall rule `Flux Remote Sensor` | first time the TCP feed is enabled | UAC once (persisted flag prevents re-prompt) | TCP 5199, inbound, allow, **private** profile — mirrors the C# installer |
| Run at Windows startup | user toggles it | none | `HKCU\…\Run` value; removable |
| Run the verified installer | user clicks Download on an available update | installer's own UAC | only after SHA-256 verification |

## Removed / hardened vs. the C# original
- **MAS (Microsoft Activation Scripts) removed.** It is a Windows-activation
  bypass that Defender classifies as a HackTool, and ran remote code as admin.
  Gone entirely.
- **Chris Titus utility no longer executes remote code.** The button now opens
  the official website in the browser; the user reviews and runs it themselves.
  No more `irm … | iex` and no elevated PowerShell helper.
- **Updater no longer blindly executes** the downloaded binary (SHA-256 gate).

## Why a clean build can still be flagged (and the fix)
`Trojan:Win32/Wacatac.C!ml` etc. are **machine-learning heuristics**. Unsigned
executables that perform installer-like actions (install/modify a service,
change the firewall, write Run keys, fetch + launch a binary) match the model
even when benign. Code changes reduce the trigger surface but the real,
industry-standard fixes are:
1. **Authenticode code-sign** the widget `.exe` and the installer (OV/EV cert),
   and submit false-positive reports to Microsoft to build SmartScreen
   reputation. An unsigned installer will keep tripping ML models regardless.
2. **Publish `SHA256SUMS`** with every release (also required by the updater).
3. Keep avoiding download-and-execute and `iex`-style patterns (done).

## Skin loader (data-only)
- User skins live as JSON in `%APPDATA%\Flux\skins\*.json`. They are
  **pure data** — geometry numbers + a `border_src` enum — deserialized with
  serde, **range-clamped** (radii/borders/spacing/alpha all bounded), and
  **cannot shadow a built-in skin name**. No code, scripts, or DLLs are ever
  loaded or executed from a skin file. Unparseable/invalid files are skipped.
- The "Skins folder" button opens that directory (creating it with a commented
  example on first use). Loaded once at startup.

## Future surfaces to keep in scope
- **Skin catalog download:** if skins are ever fetched from GitHub, keep the same
  data-only contract — download JSON, parse + clamp, never execute. (Not yet
  implemented; only local-folder install exists today.)

## Reporting
Open a private security advisory on the GitHub repository rather than a public
issue.
