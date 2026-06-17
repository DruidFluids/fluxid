# banner.ps1 — print a teal project banner (and set the terminal title) so you
# can tell at a glance which window/terminal you're in.
#
#   powershell -ExecutionPolicy Bypass -File scripts\banner.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\banner.ps1 -Name "my-label"
#
# Auto-show it whenever you open this repo by adding a line to your PowerShell
# profile (see the note printed at the bottom, or run with -Install).

param(
    [string]$Name,
    [switch]$Install
)

# Default label: "<repo-folder> — <git branch>".
if (-not $Name) {
    $proj = Split-Path -Leaf (Get-Location)
    $branch = $null
    try { $branch = (git rev-parse --abbrev-ref HEAD 2>$null) } catch {}
    if ($branch -and $branch -ne 'HEAD') { $Name = "$proj  $([char]0x2014)  $branch" }
    else { $Name = $proj }
}

$label = " $Name "
$width = try { [Console]::WindowWidth } catch { 80 }
if (-not $width -or $width -lt 24) { $width = 80 }

# Centre the label box with teal rules extending to both edges.
$dash = [char]0x2500   # ─
$pad  = [Math]::Max(2, [int](($width - $label.Length) / 2))
$leftRule  = ([string]$dash * $pad)
$rightRule = ([string]$dash * [Math]::Max(2, $width - $label.Length - $pad))

Write-Host ""
Write-Host -NoNewline $leftRule -ForegroundColor Cyan
Write-Host -NoNewline $label -ForegroundColor Black -BackgroundColor Cyan
Write-Host $rightRule -ForegroundColor Cyan
Write-Host ([string]$dash * $width) -ForegroundColor Cyan
Write-Host ""

# Also set the window title so the right project shows in the taskbar / Alt-Tab.
try { $Host.UI.RawUI.WindowTitle = $Name } catch {}

if ($Install) {
    $line = ". `"$PSCommandPath`""
    if (-not (Test-Path $PROFILE)) { New-Item -ItemType File -Path $PROFILE -Force | Out-Null }
    if (-not (Select-String -Path $PROFILE -SimpleMatch 'scripts\banner.ps1' -Quiet)) {
        Add-Content -Path $PROFILE -Value "`n# Flux project banner`nif (Test-Path '$PSCommandPath') { $line }"
        Write-Host "Added the banner to your PowerShell profile ($PROFILE)." -ForegroundColor Green
    } else {
        Write-Host "Banner already in your PowerShell profile." -ForegroundColor Yellow
    }
} else {
    Write-Host "Tip: run with -Install to show this automatically in new PowerShell sessions." -ForegroundColor DarkGray
}
