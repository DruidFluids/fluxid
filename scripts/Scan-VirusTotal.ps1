# Scan-VirusTotal.ps1 — upload an installer to VirusTotal and print the scan
# result + permalink, for inclusion in a release.
#
#   $env:VT_API_KEY = "<your key>"
#   powershell -ExecutionPolicy Bypass -File scripts\Scan-VirusTotal.ps1 -File dist\flux-setup-v1.0.0.exe
#
# The permalink (https://www.virustotal.com/gui/file/<sha256>) is deterministic
# from the file hash, so once uploaded it is permanent. Get a free API key at
# https://www.virustotal.com/gui/my-apikey . Never commit the key.

param(
    [Parameter(Mandatory = $true)][string]$File,
    [int]$TimeoutSec = 300
)

$key = $env:VT_API_KEY
if (-not $key) { throw "Set `$env:VT_API_KEY first (free key: https://www.virustotal.com/gui/my-apikey)." }
if (-not (Test-Path $File)) { throw "File not found: $File" }

$sha = (Get-FileHash $File -Algorithm SHA256).Hash.ToLower()
$permalink = "https://www.virustotal.com/gui/file/$sha"
$hdr = @{ "x-apikey" = $key }

function Get-Stats($sha) {
    try {
        $r = Invoke-RestMethod -Method Get -Uri "https://www.virustotal.com/api/v3/files/$sha" -Headers $hdr -ErrorAction Stop
        return $r.data.attributes.last_analysis_stats
    } catch { return $null }
}

$stats = Get-Stats $sha
if (-not $stats) {
    Write-Host "Uploading $([System.IO.Path]::GetFileName($File)) to VirusTotal..." -ForegroundColor Cyan
    # PowerShell 5.1's Invoke-RestMethod has no -Form, so use the bundled curl.exe
    # for the multipart upload (files <= 32 MB use the simple endpoint).
    $curl = "$env:SystemRoot\System32\curl.exe"
    & $curl -s -X POST "https://www.virustotal.com/api/v3/files" -H "x-apikey: $key" -F "file=@$File" | Out-Null
    Write-Host "  uploaded; waiting for analysis to complete..." -ForegroundColor Cyan
    # The file hash is deterministic, so poll the file endpoint directly until the
    # engines finish (more robust than the /analyses/<id> endpoint).
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $done = $false
    do {
        Start-Sleep -Seconds 15
        $stats = Get-Stats $sha
        $count = 0
        if ($stats) { $count = $stats.malicious + $stats.suspicious + $stats.undetected + $stats.harmless }
        if ($count -gt 0) { $done = $true }
        Write-Host "  engines reported so far: $count"
    } while (-not $done -and (Get-Date) -lt $deadline)
}

if ($stats) {
    $total = $stats.malicious + $stats.suspicious + $stats.undetected + $stats.harmless
    Write-Host ""
    Write-Host "=== VirusTotal result ===" -ForegroundColor Green
    Write-Host ("  Detections: {0} / {1}  (malicious {0}, suspicious {2})" -f $stats.malicious, $total, $stats.suspicious)
    Write-Host "  SHA-256:    $sha"
    Write-Host "  Permalink:  $permalink"
    Write-Host ""
    Write-Host "Markdown for the release notes / README:" -ForegroundColor Cyan
    $dash = [char]0x2014
    $tick = [char]0x60
    Write-Host "- **$($stats.malicious)/$total on VirusTotal** $dash [view the scan]($permalink). SHA-256 $tick$sha$tick."
} else {
    Write-Host "Could not retrieve stats yet. Try again shortly: $permalink" -ForegroundColor Yellow
}
