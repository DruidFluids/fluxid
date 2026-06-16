//! GitHub-release update checking + verified installer download.
//!
//! Security posture: HTTPS-only, and the downloaded installer is **never run
//! unless its SHA-256 matches a checksum published alongside the release**
//! (a `<installer>.exe.sha256` or `SHA256SUMS` asset). If no checksum is
//! published, the update is refused rather than executed — defence in depth on
//! top of TLS, so a compromised release/account can't push an unverified binary.

use sha2::{Digest, Sha256};
use std::time::Duration;

const API_URL: &str = "https://api.github.com/repos/DruidFluids/fluxid/releases/latest";
/// Human-facing page for the latest release (the "view on GitHub" link).
pub const RELEASES_URL: &str = "https://github.com/DruidFluids/fluxid/releases/latest";

/// A newer release that passed version comparison and is ready to download.
#[derive(Debug, Clone)]
pub struct PendingUpdate {
    pub version: String,
    pub changelog: String,
    pub url: String,
    /// Expected SHA-256 (hex) from the release, if one was published.
    pub sha256: Option<String>,
}

/// Outcome of a "Check now" / auto check.
#[derive(Debug, Clone)]
pub enum CheckResult {
    UpToDate,
    Available(PendingUpdate),
    Failed(String),
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("fluxid-updater")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

pub async fn check(current: String) -> CheckResult {
    match check_inner(&current).await {
        Ok(Some(update)) => CheckResult::Available(update),
        Ok(None) => CheckResult::UpToDate,
        Err(e) => CheckResult::Failed(e),
    }
}

async fn check_inner(current: &str) -> Result<Option<PendingUpdate>, String> {
    let client = client()?;
    let resp = client
        .get(API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let tag = json["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
    let body = json["body"].as_str().unwrap_or("").to_string();
    if parse_version(&tag) <= parse_version(current) {
        return Ok(None);
    }

    // Locate the .exe asset and any published checksum asset.
    let mut exe_url = String::new();
    let mut exe_name = String::new();
    let mut sha_url = String::new();
    if let Some(assets) = json["assets"].as_array() {
        for a in assets {
            let name = a["name"].as_str().unwrap_or("");
            if exe_url.is_empty() && name.to_lowercase().ends_with(".exe") {
                exe_url = a["browser_download_url"].as_str().unwrap_or("").to_string();
                exe_name = name.to_string();
            }
        }
        if !exe_name.is_empty() {
            let want = format!("{}.sha256", exe_name).to_lowercase();
            for a in assets {
                let name = a["name"].as_str().unwrap_or("").to_lowercase();
                if name == want || name == "sha256sums" || name == "sha256sums.txt" {
                    sha_url = a["browser_download_url"].as_str().unwrap_or("").to_string();
                    break;
                }
            }
        }
    }
    if exe_url.is_empty() {
        return Ok(None);
    }

    let sha256 = if sha_url.is_empty() {
        None
    } else {
        fetch_checksum(&client, &sha_url, &exe_name).await
    };

    Ok(Some(PendingUpdate { version: tag, changelog: body, url: exe_url, sha256 }))
}

/// Download a checksum asset and extract the hex digest for `exe_name`.
/// Accepts both a bare-hash file and `SHA256SUMS`-style (`<hash>  <file>`) lines.
async fn fetch_checksum(client: &reqwest::Client, url: &str, exe_name: &str) -> Option<String> {
    let text = client.get(url).send().await.ok()?.text().await.ok()?;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next().unwrap_or("");
        if !(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())) {
            continue;
        }
        match parts.next() {
            None => return Some(hash.to_string()), // single-hash file
            Some(file) => {
                if file.trim_start_matches('*').eq_ignore_ascii_case(exe_name) {
                    return Some(hash.to_string());
                }
            }
        }
    }
    None
}

fn parse_version(v: &str) -> (u32, u32, u32) {
    let mut it = v.split(['.', '-']).filter_map(|s| s.parse::<u32>().ok());
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// Fetch the latest release's (version, changelog body) for display — no
/// version comparison, so it works even when already up to date. Used to fill
/// the Updates card with the newest release notes.
pub async fn latest_release() -> Result<(String, String), String> {
    let client = client()?;
    let resp = client
        .get(API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = json["tag_name"].as_str().unwrap_or("").to_string();
    let body = json["body"].as_str().unwrap_or("").trim().to_string();
    Ok((tag, body))
}

/// Filter a release body down to changelog bullet lines (matches C#).
pub fn changelog_bullets(body: &str) -> String {
    let bullets: Vec<&str> = body
        .lines()
        .map(|l| l.trim())
        .filter(|t| t.starts_with("- ") || t.starts_with("* "))
        .collect();
    if bullets.is_empty() {
        body.trim().to_string()
    } else {
        bullets.join("\n")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Download the installer to %TEMP%, **verify its SHA-256**, then launch it
/// silently. Refuses to run if the checksum is missing or mismatched. The
/// caller exits the app on success.
pub async fn download_and_launch(url: String, expected_sha256: Option<String>) -> Result<(), String> {
    let expected = expected_sha256
        .ok_or_else(|| "No checksum published for this release — update aborted for safety".to_string())?;

    let resp = client()?.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    let actual = sha256_hex(&bytes);
    if !actual.eq_ignore_ascii_case(expected.trim()) {
        return Err("Integrity check failed (checksum mismatch) — update aborted".into());
    }

    let fname = url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("fluxid-setup.exe");
    let path = std::env::temp_dir().join(fname);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(&path)
            .args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
