# Notes / pending decisions

Things that need a product decision from you — nothing is blocked, these are
trade-offs I didn't want to make unilaterally.

## CPU temperature accuracy — RESOLVED (optional PawnIO opt-in)
Re-added the optional CPU sensor driver, faithful to C#. The user opts in from
**Settings → Tiles → the "i" / Active-Inactive chip next to CPU temperature**.

- **Install (secure):** never bundles the driver; downloads the official signed
  PawnIO installer, verifies it with `WinVerifyTrust` (trusted chain +
  revocation) before running, then one silent elevated install. Uninstall via
  the driver's own registry key. (`cpu_driver.rs`)
- **Read (self-contained):** bundles the officially-signed PawnIO modules
  (LGPL-2.1) — `IntelMSR.bin` (all Intel), `AMDFamily17.bin` (all AMD Zen 1–5)
  — and reads the CPU's thermal MSR/SMN directly via `PawnIOLib.dll`. Decode is
  a faithful port of LibreHardwareMonitor. (`fluid-sensor/src/pawnio.rs`)
- Preferred source on Windows; LHM-WMI and ACPI remain fallbacks. Re-probes
  live after install/uninstall (no restart).

CPU-temp source order (Windows): PawnIO → sysinfo components → LHM/OHM WMI →
ACPI thermal zone (rejects <20 °C so a chipset/ambient zone never shows as the
CPU die).

**Needs your validation (hardware-specific):** install PawnIO via the dialog on
your 9950X3D and confirm the reading matches HWiNFO/Ryzen Master. The AMD Zen5
decode is ported from LHM but unverified on real silicon until you test.

**Pre-Zen AMD (FX/Phenom) & pre-2011 Intel:** not yet covered natively (would
need their family modules + decode); they fall back to LHM-WMI/ACPI.

Related limitation: **CPU clock** via sysinfo is the base/nominal clock on
Windows (e.g. a static 4300 MHz), not the live boosting frequency — there's no
clean driver-free live-clock API. CPU **usage** does update correctly.

## Settings UI redesign — DONE (tabs)
Rebuilt the Settings window as **tabs** (Tiles · Appearance · Behavior ·
Sensors · Remote · Updates) — one category at a time. The window resizes to
each tab so it stays compact, and there is **no scrollbar**. This diverges from
the C# single-pane layout deliberately, per your "less at once / no scrollbar /
not too big" feedback. If you'd prefer a different grouping or tab order, say so.

## Done recently
- Settings redesigned into compact tabs; no scrollbar; per-tab sizing.
- Secondary windows skip the taskbar (only the widget shows one entry).
- Light-theme readability fixed everywhere (field backgrounds + Alerts/colour-hex inputs + themed sliders).
- All window titles flush in the top-left corner.
- CPU temperature °C/°F moved to the top of Settings.
- Appearance changes (theme/skin/colours/fonts) persist immediately.
- Popup/sub-windows remember their last position.
- "colours" → "colors" in user-facing text.
