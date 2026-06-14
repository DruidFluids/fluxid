# Notes / pending decisions

Things that need a product decision from you — nothing is blocked, these are
trade-offs I didn't want to make unilaterally.

## CPU temperature accuracy (needs your call)
Accurate CPU **package/die** temperature on Windows requires a kernel-level
sensor driver (RAPL/MSR access). The old C# app used PawnIO for this. That
conflicts directly with the "zero security issues / no kernel driver" goal.

What the Rust port does now, in order of preference:
1. **sysinfo components** — only populated if a hardware-monitor driver is already feeding the OS.
2. **LibreHardwareMonitor / OpenHardwareMonitor WMI** — accurate CPU Package/core temp, *if you run one of those apps in the background* (no driver shipped by us). **Added today.**
3. **ACPI thermal zone (MSAcpi)** — coarse fallback. On your machine this reports ~17 °C, which is an ambient/chipset zone, **not** the CPU die. It now **rejects readings below 20 °C** (impossible for a CPU die) so the tile shows "—" instead of a misleading "17 °C". So: with nothing else available, CPU temp will read "—" until you run a hardware monitor (option a).

Options:
- **(a)** Run LibreHardwareMonitor in the background → fluidMonitor will read accurate temps automatically. Zero security cost. *(Recommended.)*
- **(b)** Accept that CPU temp may be inaccurate/absent without a helper.
- **(c)** Ship a signed kernel sensor driver → accurate, but a security/AV surface you've said you want to avoid.

Related limitation: **CPU clock** via sysinfo is the base/nominal clock on
Windows (e.g. a static 4300 MHz), not the live boosting frequency — there's no
clean driver-free live-clock API. CPU **usage** does update correctly.

## Settings UI — making it less overwhelming (your call on direction)
The Settings window is a dense two-column scroll of ~9 sections. It's complete
but a lot at once. Ranked proposals (I held off implementing a big redesign
since it trades against the earlier "match C# exactly" goal — tell me which to
pursue):

- **A. Category tabs / sidebar** *(biggest win, biggest change)* — a left rail
  (General · Appearance · Behavior · Sensors · Remote · Updates); show one group
  at a time. ~70% less on screen, smaller window, but diverges from C#'s single
  pane and adds a click to reach a setting.
- **B. Collapsible sections (accordion)** *(recommended balance)* — a chevron on
  each section header; remember collapsed state; default-collapse the rarely-used
  ones (Remote, Updates, Disk, Font). Keeps the familiar single pane; user
  controls density. Remote Monitoring already works this way — just extend it.
- **C. Progressive "Advanced" disclosure** — hide the fine-tune sliders
  (font-size offsets, arrow size/spacing, R/W size/spacing, snap distance, muted
  contrast) behind a small "Advanced ▾" per section. Removes ~8 sliders from the
  default view; pairs well with B.
- **D. Visual grouping into cards** *(low risk, cosmetic)* — wrap each section in
  a faint rounded card (like the Skins/Updates boxes already are) so the eye
  chunks sections. Helps a bit; adds some visual lines.
- **E. Spacing/typography polish** *(safe)* — more breathing room between
  sections, lighter value labels, slightly larger headers.

My suggestion: **B + C** gives the calmest result with the least disruption and
no hard divergence from C#. Say the word and I'll build it.

## Already done while you were away
- Light-theme readability fixed (field backgrounds + Alerts/colour-hex inputs).
- All window titles flush in the top-left corner.
- CPU temperature °C/°F moved to the top of Settings.
- Popup/sub-windows remember their last position.
- "colours" → "colors" in user-facing text.
