//! GitHub-release update checking + installer download. Mirrors the C#
//! `UpdateService`: query the latest release, compare semver, and (on request)
//! download the .exe asset and launch it silently.

use std::time::Duration;

const API_URL: &str = "https://api.github.com/repos/DruidFluids/fluidMonitor/releases/latest";

/// Outcome of a "Check now" / auto check.
#[derive(Debug, Clone)]
pub enum CheckResult {
    UpToDate,
    Available { version: String, changelog: String, url: String },
    Failed(String),
}

pub async fn check(current: String) -> CheckResult {
    match check_inner(&current).await {
        Ok(Some((version, changelog, url))) => CheckResult::Available { version, changelog, url },
        Ok(None) => CheckResult::UpToDate,
        Err(e) => CheckResult::Failed(e),
    }
}

async fn check_inner(current: &str) -> Result<Option<(String, String, String)>, String> {
    let client = reqwest::Client::builder()
        .user_agent("fluidMonitor-updater")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
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

    // Find the first .exe asset.
    let mut url = String::new();
    if let Some(assets) = json["assets"].as_array() {
        for a in assets {
            let name = a["name"].as_str().unwrap_or("");
            if name.to_lowercase().ends_with(".exe") {
                url = a["browser_download_url"].as_str().unwrap_or("").to_string();
                break;
            }
        }
    }
    if url.is_empty() {
        return Ok(None);
    }
    Ok(Some((tag, body, url)))
}

fn parse_version(v: &str) -> (u32, u32, u32) {
    let mut it = v.split(['.', '-']).filter_map(|s| s.parse::<u32>().ok());
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
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

/// Download the installer to %TEMP% and launch it silently. The installer
/// handles stop/uninstall/reinstall; the caller exits the app on success.
pub async fn download_and_launch(url: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("fluidMonitor-updater")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let fname = url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("fluidMonitor-setup.exe");
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
