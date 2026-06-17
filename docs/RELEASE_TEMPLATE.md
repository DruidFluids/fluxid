# Flux vX.Y.Z

<One- or two-sentence summary of the release.>

## Highlights

- **Short title** — what changed and why it matters.
- **Short title** — ...

## Security

- No telemetry.
- Verified updates — the in-app updater only runs a downloaded installer whose SHA-256 matches the published checksum.
- Scanned on VirusTotal: **N / M** — [view the scan](https://www.virustotal.com/gui/file/<sha256>). <If any engine flags it, note it's a false positive from an unsigned-binary heuristic.>
- The build is unsigned; Windows SmartScreen shows a one-time "Windows protected your PC" prompt (More info, then Run anyway).

## Install

Existing users: open Settings → Updates → Check now to update in place. Or download flux-setup-vX.Y.Z.exe below and run it. To verify it first, run Get-FileHash on the file and compare against:

SHA-256: <sha256>

<!--
Changelog format — keep every release consistent with this template:
  * Headers, in order: ## Highlights, ## Security, ## Install (H2, exactly these titles).
  * Title line is always `# Flux vX.Y.Z`.
  * Highlights are bullet lines starting with `- ` (the in-app updater extracts these
    as the changelog, so every user-visible change must be a top-level bullet).
  * Bold the lead phrase of each Highlight with **...**; use an em dash before the detail.
  * VirusTotal line: `**N / M**` with no extra words like "clean"; link the scan permalink.
  * Generate the Security VT/SHA lines with scripts/Scan-VirusTotal.ps1.
-->
