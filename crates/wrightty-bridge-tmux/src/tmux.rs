//! Functions that shell out to `tmux` CLI commands.

use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct TmuxPane {
    /// Full pane target in `<session>:<window>.<pane>` format, e.g. `main:0.1`
    pub target: String,
    pub session_name: String,
    pub window_index: u32,
    pub pane_index: u32,
    pub cols: u16,
    pub rows: u16,
    pub title: String,
    pub active: bool,
    pub pid: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum TmuxError {
    #[error("tmux command failed: {0}")]
    CommandFailed(String),
    #[error("failed to parse tmux output: {0}")]
    ParseError(String),
    #[error("pane {0} not found")]
    PaneNotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

fn tmux_cmd(args: &[&str]) -> Command {
    let cmd_str = std::env::var("TMUX_CMD").unwrap_or_else(|_| "tmux".to_string());
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    let (program, prefix_args) = parts.split_first().expect("TMUX_CMD must not be empty");

    let mut cmd = Command::new(program);
    for arg in prefix_args {
        cmd.arg(arg);
    }
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

/// Check if tmux server is reachable.
pub async fn health_check() -> Result<(), TmuxError> {
    let output = tmux_cmd(&["list-sessions"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(format!("tmux not reachable: {stderr}")));
    }

    Ok(())
}

/// List all panes across all sessions.
pub async fn list_panes() -> Result<Vec<TmuxPane>, TmuxError> {
    // Format: session_name:window_index.pane_index|cols|rows|title|active|pid
    let format = "#{session_name}:#{window_index}.#{pane_index}|#{pane_width}|#{pane_height}|#{pane_title}|#{pane_active}|#{pane_pid}";
    let output = tmux_cmd(&["list-panes", "-a", "-F", format]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut panes = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(6, '|').collect();
        if parts.len() < 6 {
            continue;
        }
        let target = parts[0].to_string();
        // Parse session:window.pane from target
        let (session_name, window_pane) = target
            .split_once(':')
            .ok_or_else(|| TmuxError::ParseError(format!("bad target: {target}")))?;
        let (window_str, pane_str) = window_pane
            .split_once('.')
            .ok_or_else(|| TmuxError::ParseError(format!("bad target: {target}")))?;

        panes.push(TmuxPane {
            target: target.clone(),
            session_name: session_name.to_string(),
            window_index: window_str.parse().unwrap_or(0),
            pane_index: pane_str.parse().unwrap_or(0),
            cols: parts[1].parse().unwrap_or(80),
            rows: parts[2].parse().unwrap_or(24),
            title: parts[3].to_string(),
            active: parts[4] == "1",
            pid: parts[5].trim().parse().unwrap_or(0),
        });
    }

    Ok(panes)
}

/// Get visible screen text for a pane.
pub async fn capture_pane(target: &str) -> Result<String, TmuxError> {
    let output = tmux_cmd(&["capture-pane", "-t", target, "-p"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Get scrollback buffer for a pane.
pub async fn capture_scrollback(target: &str) -> Result<String, TmuxError> {
    // -S - means start from the beginning of history; -E - means end at current line
    let output = tmux_cmd(&["capture-pane", "-t", target, "-p", "-S", "-", "-E", "-"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Send literal text to a pane (no key name interpretation).
pub async fn send_text(target: &str, text: &str) -> Result<(), TmuxError> {
    let output = tmux_cmd(&["send-keys", "-t", target, "-l", text]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Send a key sequence to a pane (tmux key name format, e.g. "Enter", "C-c").
pub async fn send_key(target: &str, key: &str) -> Result<(), TmuxError> {
    let output = tmux_cmd(&["send-keys", "-t", target, key]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Create a new window and return its pane target.
pub async fn new_window(session: Option<&str>) -> Result<String, TmuxError> {
    let mut args = vec!["new-window", "-P", "-F",
        "#{session_name}:#{window_index}.#{pane_index}"];
    if let Some(s) = session {
        args.push("-t");
        args.push(s);
    }
    let output = tmux_cmd(&args).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim().to_string())
}

/// Kill a pane.
pub async fn kill_pane(target: &str) -> Result<(), TmuxError> {
    let output = tmux_cmd(&["kill-pane", "-t", target]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Resize a pane to exact dimensions.
pub async fn resize_pane(target: &str, cols: u16, rows: u16) -> Result<(), TmuxError> {
    let cols_str = cols.to_string();
    let rows_str = rows.to_string();
    let output = tmux_cmd(&[
        "resize-pane", "-t", target,
        "-x", &cols_str,
        "-y", &rows_str,
    ])
    .output()
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TmuxError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Find a pane by its target string.
pub async fn find_pane(target: &str) -> Result<TmuxPane, TmuxError> {
    let panes = list_panes().await?;
    panes
        .into_iter()
        .find(|p| p.target == target)
        .ok_or_else(|| TmuxError::PaneNotFound(target.to_string()))
}
