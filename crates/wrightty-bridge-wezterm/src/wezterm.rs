//! Functions that shell out to `wezterm cli` and parse results.

use serde::Deserialize;
use tokio::process::Command;

/// A pane entry as returned by `wezterm cli list --format json`.
#[derive(Debug, Clone, Deserialize)]
pub struct WezTermPane {
    pub pane_id: u64,
    pub tab_id: u64,
    pub window_id: u64,
    pub workspace: String,
    pub size: PaneSize,
    pub title: String,
    pub cwd: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_zoomed: bool,
    #[serde(default)]
    pub cursor_x: u64,
    #[serde(default)]
    pub cursor_y: u64,
    #[serde(default)]
    pub cursor_shape: Option<String>,
    #[serde(default)]
    pub cursor_visibility: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaneSize {
    pub rows: u16,
    pub cols: u16,
    #[serde(default)]
    pub pixel_width: u32,
    #[serde(default)]
    pub pixel_height: u32,
    #[serde(default)]
    pub dpi: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum WezTermError {
    #[error("wezterm cli failed: {0}")]
    CommandFailed(String),
    #[error("failed to parse wezterm output: {0}")]
    ParseError(String),
    #[error("pane {0} not found")]
    PaneNotFound(u64),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// List all panes via `wezterm cli list --format json`.
pub async fn list_panes() -> Result<Vec<WezTermPane>, WezTermError> {
    let output = Command::new("wezterm")
        .args(["cli", "list", "--format", "json"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WezTermError::CommandFailed(stderr.into_owned()));
    }

    let panes: Vec<WezTermPane> = serde_json::from_slice(&output.stdout)
        .map_err(|e| WezTermError::ParseError(e.to_string()))?;

    Ok(panes)
}

/// Get the text content of a pane via `wezterm cli get-text --pane-id N`.
pub async fn get_text(pane_id: u64) -> Result<String, WezTermError> {
    let output = Command::new("wezterm")
        .args(["cli", "get-text", "--pane-id", &pane_id.to_string()])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WezTermError::CommandFailed(stderr.into_owned()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Send text to a pane via `wezterm cli send-text --pane-id N --no-paste "text"`.
pub async fn send_text(pane_id: u64, text: &str) -> Result<(), WezTermError> {
    let output = Command::new("wezterm")
        .args([
            "cli",
            "send-text",
            "--pane-id",
            &pane_id.to_string(),
            "--no-paste",
            text,
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WezTermError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Spawn a new pane via `wezterm cli spawn`. Returns the new pane ID.
pub async fn spawn_pane() -> Result<u64, WezTermError> {
    let output = Command::new("wezterm")
        .args(["cli", "spawn"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WezTermError::CommandFailed(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pane_id: u64 = stdout
        .trim()
        .parse()
        .map_err(|e: std::num::ParseIntError| WezTermError::ParseError(e.to_string()))?;

    Ok(pane_id)
}

/// Kill a pane via `wezterm cli kill-pane --pane-id N`.
pub async fn kill_pane(pane_id: u64) -> Result<(), WezTermError> {
    let output = Command::new("wezterm")
        .args(["cli", "kill-pane", "--pane-id", &pane_id.to_string()])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WezTermError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Find a specific pane by ID from the list output.
pub async fn find_pane(pane_id: u64) -> Result<WezTermPane, WezTermError> {
    let panes = list_panes().await?;
    panes
        .into_iter()
        .find(|p| p.pane_id == pane_id)
        .ok_or(WezTermError::PaneNotFound(pane_id))
}
