//! Version check — non-blocking check against crates.io for newer versions.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours
const CRATE_NAME: &str = "wrightty";

fn cache_path() -> Option<PathBuf> {
    dirs_next().map(|d| d.join("wrightty_version_check"))
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).map(|h| h.join(".cache"))
}

/// Spawn a background task that checks for a newer version and prints a message.
/// Does NOT block the main thread. Caches results for 24 hours.
pub fn check_in_background() {
    tokio::spawn(async {
        if let Some(msg) = check_for_update().await {
            eprintln!("\n{msg}");
        }
    });
}

async fn check_for_update() -> Option<String> {
    let cache = cache_path()?;
    let _ = fs::create_dir_all(cache.parent()?);

    // Check cache
    if let Ok(contents) = fs::read_to_string(&cache) {
        let parts: Vec<&str> = contents.splitn(2, '\n').collect();
        if parts.len() == 2 {
            if let Ok(ts) = parts[0].parse::<u64>() {
                let cached_at = SystemTime::UNIX_EPOCH + Duration::from_secs(ts);
                if SystemTime::now().duration_since(cached_at).unwrap_or_default() < CHECK_INTERVAL {
                    let latest = parts[1].trim();
                    return format_update_message(latest);
                }
            }
        }
    }

    // Fetch latest version from crates.io
    let latest = fetch_latest_version().await?;

    // Cache result
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = fs::write(&cache, format!("{now}\n{latest}"));

    format_update_message(&latest)
}

fn format_update_message(latest: &str) -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    if latest != current && version_is_newer(latest, current) {
        Some(format!(
            "\x1b[33mA new version of wrightty is available: {current} → {latest}\n\
             Update with: wrightty upgrade\x1b[0m"
        ))
    } else {
        None
    }
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    parse(latest) > parse(current)
}

async fn fetch_latest_version() -> Option<String> {
    let url = format!("https://crates.io/api/v1/crates/{CRATE_NAME}");
    let resp = tokio::time::timeout(Duration::from_secs(3), async {
        reqwest_lite(&url).await
    })
    .await
    .ok()?;
    resp.and_then(|body| {
        let v: serde_json::Value = serde_json::from_str(&body).ok()?;
        v["crate"]["newest_version"].as_str().map(String::from)
    })
}

/// Minimal HTTP GET using tokio TCP — no extra deps.
async fn reqwest_lite(url: &str) -> Option<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let host = "crates.io";
    let path = url.strip_prefix("https://crates.io")?;

    let mut stream = TcpStream::connect(format!("{host}:443")).await.ok()?;

    // For simplicity, use HTTP (not HTTPS) via the API which redirects.
    // Actually crates.io requires HTTPS. Let's just use a simple TCP approach
    // with the plain HTTP API endpoint that returns JSON.
    drop(stream);

    // Use a spawned process instead for simplicity (curl is ubiquitous)
    let output = tokio::process::Command::new("curl")
        .args(["-sf", "-H", "User-Agent: wrightty-cli", "--max-time", "3", url])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

/// Run the upgrade command.
pub async fn upgrade() -> anyhow::Result<()> {
    println!("Checking for updates...");

    let latest = fetch_latest_version().await;
    let current = env!("CARGO_PKG_VERSION");

    match latest {
        Some(ref v) if version_is_newer(v, current) => {
            println!("Upgrading wrightty {current} → {v}...");
            println!();
            let status = tokio::process::Command::new("cargo")
                .args(["install", "wrightty"])
                .status()
                .await?;
            if status.success() {
                println!("\nUpgraded to wrightty {v}");
            } else {
                anyhow::bail!("Upgrade failed. Try manually: cargo install wrightty");
            }
        }
        Some(_) => {
            println!("Already up to date (v{current}).");
        }
        None => {
            println!("Could not check for updates. Current version: {current}");
        }
    }

    Ok(())
}
