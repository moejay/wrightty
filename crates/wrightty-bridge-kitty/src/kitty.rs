//! Functions that shell out to `kitty @` remote control commands.
//!
//! Requires kitty to be started with `allow_remote_control yes` in kitty.conf,
//! or launched with `--listen-on unix:/path/to/socket`.
//!
//! Set `KITTY_LISTEN_ON` to the socket path if kitty is not using the default.

use serde::Deserialize;
use tokio::process::Command;

/// A kitty window as returned by `kitty @ ls`.
#[derive(Debug, Clone, Deserialize)]
pub struct KittyWindow {
    pub id: u64,
    pub title: String,
    pub is_focused: bool,
    pub columns: u16,
    pub lines: u16,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub cwd: Option<String>,
}

/// Intermediate structures for parsing `kitty @ ls` JSON output.
#[derive(Debug, Deserialize)]
struct KittyOsWindow {
    tabs: Vec<KittyTab>,
}

#[derive(Debug, Deserialize)]
struct KittyTab {
    windows: Vec<KittyWindowRaw>,
}

#[derive(Debug, Deserialize)]
struct KittyWindowRaw {
    id: u64,
    title: String,
    is_focused: bool,
    columns: u16,
    lines: u16,
    #[serde(default)]
    pid: Option<u32>,
    foreground_processes: Vec<KittyProcess>,
}

#[derive(Debug, Deserialize)]
struct KittyProcess {
    pid: u32,
    cwd: String,
}

#[derive(Debug, thiserror::Error)]
pub enum KittyError {
    #[error("kitty command failed: {0}")]
    CommandFailed(String),
    #[error("failed to parse kitty output: {0}")]
    ParseError(String),
    #[error("window {0} not found")]
    WindowNotFound(u64),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

fn kitty_cmd(args: &[&str]) -> Command {
    let cmd_str = std::env::var("KITTY_CMD").unwrap_or_else(|_| "kitty".to_string());
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    let (program, prefix_args) = parts.split_first().expect("KITTY_CMD must not be empty");

    let mut cmd = Command::new(program);
    for arg in prefix_args {
        cmd.arg(arg);
    }
    // Always prepend "@" subcommand for remote control
    cmd.arg("@");

    // If KITTY_LISTEN_ON is set, pass it as the --to argument
    if let Ok(socket) = std::env::var("KITTY_LISTEN_ON") {
        cmd.arg("--to");
        cmd.arg(socket);
    }

    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

/// Check if kitty is reachable via remote control.
pub async fn health_check() -> Result<(), KittyError> {
    let output = kitty_cmd(&["ls"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(format!("kitty not reachable: {stderr}")));
    }

    let os_windows: Vec<KittyOsWindow> = serde_json::from_slice(&output.stdout)
        .map_err(|e| KittyError::ParseError(e.to_string()))?;

    let total_windows: usize = os_windows
        .iter()
        .flat_map(|ow| ow.tabs.iter())
        .map(|t| t.windows.len())
        .sum();

    if total_windows == 0 {
        return Err(KittyError::CommandFailed("kitty has no windows".to_string()));
    }

    Ok(())
}

/// List all windows across all OS windows and tabs.
pub async fn list_windows() -> Result<Vec<KittyWindow>, KittyError> {
    let output = kitty_cmd(&["ls"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    let os_windows: Vec<KittyOsWindow> = serde_json::from_slice(&output.stdout)
        .map_err(|e| KittyError::ParseError(e.to_string()))?;

    let windows: Vec<KittyWindow> = os_windows
        .into_iter()
        .flat_map(|ow| ow.tabs)
        .flat_map(|t| t.windows)
        .map(|w| {
            let cwd = w.foreground_processes.first().map(|p| p.cwd.clone());
            let pid = w.pid.or_else(|| w.foreground_processes.first().map(|p| p.pid));
            KittyWindow {
                id: w.id,
                title: w.title,
                is_focused: w.is_focused,
                columns: w.columns,
                lines: w.lines,
                pid,
                cwd,
            }
        })
        .collect();

    Ok(windows)
}

/// Get screen text for a window via `kitty @ get-text --match id:<id> --extent screen`.
pub async fn get_text(window_id: u64) -> Result<String, KittyError> {
    let match_str = format!("id:{window_id}");
    let output = kitty_cmd(&["get-text", "--match", &match_str, "--extent", "screen"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Get scrollback text for a window.
pub async fn get_scrollback(window_id: u64) -> Result<String, KittyError> {
    let match_str = format!("id:{window_id}");
    let output = kitty_cmd(&["get-text", "--match", &match_str, "--extent", "all"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Send literal text to a window via `kitty @ send-text`.
pub async fn send_text(window_id: u64, text: &str) -> Result<(), KittyError> {
    let match_str = format!("id:{window_id}");
    let output = kitty_cmd(&["send-text", "--match", &match_str, "--", text])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Send a key sequence to a window via `kitty @ send-key`.
pub async fn send_key(window_id: u64, key: &str) -> Result<(), KittyError> {
    let match_str = format!("id:{window_id}");
    let output = kitty_cmd(&["send-key", "--match", &match_str, "--", key])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Launch a new kitty window. Returns the new window ID.
pub async fn launch_window() -> Result<u64, KittyError> {
    let output = kitty_cmd(&["launch", "--type=window"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let window_id: u64 = stdout
        .trim()
        .parse()
        .map_err(|e: std::num::ParseIntError| KittyError::ParseError(e.to_string()))?;

    Ok(window_id)
}

/// Close a kitty window.
pub async fn close_window(window_id: u64) -> Result<(), KittyError> {
    let match_str = format!("id:{window_id}");
    let output = kitty_cmd(&["close-window", "--match", &match_str]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Resize a kitty window.
pub async fn resize_window(window_id: u64, cols: u16, rows: u16) -> Result<(), KittyError> {
    let match_str = format!("id:{window_id}");
    let cols_str = cols.to_string();
    let rows_str = rows.to_string();
    let output = kitty_cmd(&[
        "resize-window",
        "--match", &match_str,
        "--width", &cols_str,
        "--height", &rows_str,
    ])
    .output()
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KittyError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Find a window by ID.
pub async fn find_window(window_id: u64) -> Result<KittyWindow, KittyError> {
    let windows = list_windows().await?;
    windows
        .into_iter()
        .find(|w| w.id == window_id)
        .ok_or(KittyError::WindowNotFound(window_id))
}
