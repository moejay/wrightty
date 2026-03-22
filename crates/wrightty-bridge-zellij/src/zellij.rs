//! Functions that shell out to `zellij` CLI action commands.
//!
//! Requires the bridge to run from within a Zellij session (ZELLIJ_SESSION_NAME must be set),
//! or set ZELLIJ_SESSION_NAME explicitly before starting the bridge.

use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ZellijSession {
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ZellijError {
    #[error("zellij command failed: {0}")]
    CommandFailed(String),
    #[error("failed to parse zellij output: {0}")]
    ParseError(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("ZELLIJ_SESSION_NAME is not set — bridge must run inside a zellij session")]
    NoSessionName,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Get the current zellij session name from the environment.
pub fn session_name() -> Result<String, ZellijError> {
    std::env::var("ZELLIJ_SESSION_NAME").map_err(|_| ZellijError::NoSessionName)
}

fn zellij_cmd(args: &[&str]) -> Command {
    let cmd_str = std::env::var("ZELLIJ_CMD").unwrap_or_else(|_| "zellij".to_string());
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    let (program, prefix_args) = parts.split_first().expect("ZELLIJ_CMD must not be empty");

    let mut cmd = Command::new(program);
    for arg in prefix_args {
        cmd.arg(arg);
    }
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

fn zellij_action_cmd(action_args: &[&str]) -> Command {
    let mut args = vec!["action"];
    args.extend_from_slice(action_args);
    zellij_cmd(&args)
}

/// Check if zellij is reachable and we are inside a session.
pub async fn health_check() -> Result<(), ZellijError> {
    // Verify ZELLIJ_SESSION_NAME is set
    let _ = session_name()?;

    // Verify zellij is installed and accessible
    let output = zellij_cmd(&["list-sessions"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(format!("zellij not reachable: {stderr}")));
    }

    Ok(())
}

/// List active zellij sessions.
pub async fn list_sessions() -> Result<Vec<ZellijSession>, ZellijError> {
    let output = zellij_cmd(&["list-sessions", "--no-formatting"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sessions: Vec<ZellijSession> = stdout
        .lines()
        .filter_map(|line| {
            let name = line.trim().split_whitespace().next()?.to_string();
            if name.is_empty() { None } else { Some(ZellijSession { name }) }
        })
        .collect();

    Ok(sessions)
}

/// Dump the visible screen to a temp file and return its contents.
pub async fn dump_screen() -> Result<String, ZellijError> {
    let tmp = tempfile::NamedTempFile::new()?;
    let path = tmp.path().to_string_lossy().into_owned();

    let output = zellij_action_cmd(&["dump-screen", &path]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    let content = tokio::fs::read_to_string(&path).await?;
    Ok(content)
}

/// Dump the full scrollback to a temp file and return its contents.
pub async fn dump_scrollback() -> Result<String, ZellijError> {
    let tmp = tempfile::NamedTempFile::new()?;
    let path = tmp.path().to_string_lossy().into_owned();

    let output = zellij_action_cmd(&["dump-screen", "--full", &path]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    let content = tokio::fs::read_to_string(&path).await?;
    Ok(content)
}

/// Send literal text to the focused pane via `zellij action write-chars`.
pub async fn write_chars(text: &str) -> Result<(), ZellijError> {
    let output = zellij_action_cmd(&["write-chars", text]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Send raw bytes to the focused pane via `zellij action write`.
///
/// `bytes` should be space-separated decimal byte values, e.g. "13" for Enter.
pub async fn write_bytes(bytes: &str) -> Result<(), ZellijError> {
    let output = zellij_action_cmd(&["write", bytes]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Open a new pane.
pub async fn new_pane() -> Result<(), ZellijError> {
    let output = zellij_action_cmd(&["new-pane"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Close the focused pane.
pub async fn close_pane() -> Result<(), ZellijError> {
    let output = zellij_action_cmd(&["close-pane"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    Ok(())
}

/// Get tab names for the current session.
pub async fn query_tab_names() -> Result<Vec<String>, ZellijError> {
    let output = zellij_action_cmd(&["query-tab-names"]).output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ZellijError::CommandFailed(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let names: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(names)
}

/// Convert a wrightty key to raw bytes for `zellij action write`.
///
/// Returns the space-separated byte values as a string.
pub fn key_to_bytes(text: &str) -> String {
    text.bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
