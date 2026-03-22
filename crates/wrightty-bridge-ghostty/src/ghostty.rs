//! Ghostty IPC client.
//!
//! Ghostty exposes a Unix domain socket for inter-process communication.
//! The socket path can be configured via the `GHOSTTY_SOCKET` environment variable,
//! defaulting to `$XDG_RUNTIME_DIR/ghostty/sock` on Linux and
//! `$TMPDIR/ghostty-<uid>.sock` on macOS.
//!
//! Messages are exchanged as newline-delimited JSON (one JSON object per line).
//! Requests use `{"type":"<command>", ...fields}` and responses mirror the
//! same structure with a `"result"` field.
//!
//! Input (text / key injection) is delegated to `xdotool` on Linux (X11) or
//! `osascript` on macOS because Ghostty does not yet expose a send-text IPC
//! call.  Set `GHOSTTY_INPUT_BACKEND=xdotool|osascript|none` to override.

use std::env;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A Ghostty window as returned by the `list_windows` IPC call.
#[derive(Debug, Clone, Deserialize)]
pub struct GhosttyWindow {
    pub id: u64,
    pub title: String,
    pub is_focused: bool,
    pub cols: u16,
    pub rows: u16,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub cwd: Option<String>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum GhosttyError {
    #[error("ghostty socket not found at {0}; is Ghostty running?")]
    SocketNotFound(String),
    #[error("ghostty IPC call failed: {0}")]
    IpcFailed(String),
    #[error("failed to parse ghostty response: {0}")]
    ParseError(String),
    #[error("window {0} not found")]
    WindowNotFound(u64),
    #[error("input backend error: {0}")]
    InputBackend(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Socket helpers
// ---------------------------------------------------------------------------

fn socket_path() -> String {
    if let Ok(p) = env::var("GHOSTTY_SOCKET") {
        return p;
    }

    #[cfg(target_os = "macos")]
    {
        let uid = unsafe { libc::getuid() };
        let tmp = env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
        return format!("{tmp}/ghostty-{uid}.sock");
    }

    // Linux default: $XDG_RUNTIME_DIR/ghostty/sock
    let runtime_dir = env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    format!("{runtime_dir}/ghostty/sock")
}

/// Open a connection to the Ghostty IPC socket.
async fn connect() -> Result<UnixStream, GhosttyError> {
    let path = socket_path();
    UnixStream::connect(&path)
        .await
        .map_err(|_| GhosttyError::SocketNotFound(path))
}

// ---------------------------------------------------------------------------
// IPC request / response
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct IpcRequest<'a, T: Serialize> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(flatten)]
    payload: T,
}

#[derive(Deserialize, Debug)]
struct IpcResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(flatten)]
    fields: serde_json::Value,
}

/// Send one IPC request and read the response line.
async fn ipc_call<T: Serialize>(
    kind: &str,
    payload: T,
) -> Result<serde_json::Value, GhosttyError> {
    let mut stream = connect().await?;

    let req = IpcRequest { kind, payload };
    let mut line = serde_json::to_string(&req)
        .map_err(|e| GhosttyError::ParseError(e.to_string()))?;
    line.push('\n');

    stream
        .write_all(line.as_bytes())
        .await
        .map_err(GhosttyError::Io)?;

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader
        .read_line(&mut resp_line)
        .await
        .map_err(GhosttyError::Io)?;

    let resp: IpcResponse = serde_json::from_str(resp_line.trim())
        .map_err(|e| GhosttyError::ParseError(format!("{e}: {resp_line}")))?;

    if let Some(err) = resp.error {
        return Err(GhosttyError::IpcFailed(err));
    }

    Ok(resp.fields)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check that Ghostty is reachable by listing windows.
pub async fn health_check() -> Result<(), GhosttyError> {
    list_windows().await?;
    Ok(())
}

/// List all Ghostty windows.
pub async fn list_windows() -> Result<Vec<GhosttyWindow>, GhosttyError> {
    #[derive(Serialize)]
    struct Empty {}
    let value = ipc_call("list_windows", Empty {}).await?;

    let windows: Vec<GhosttyWindow> = serde_json::from_value(
        value
            .get("windows")
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![])),
    )
    .map_err(|e| GhosttyError::ParseError(e.to_string()))?;

    Ok(windows)
}

/// Find a window by its numeric ID.
pub async fn find_window(window_id: u64) -> Result<GhosttyWindow, GhosttyError> {
    let windows = list_windows().await?;
    windows
        .into_iter()
        .find(|w| w.id == window_id)
        .ok_or(GhosttyError::WindowNotFound(window_id))
}

/// Open a new Ghostty window and return its ID.
pub async fn new_window() -> Result<u64, GhosttyError> {
    #[derive(Serialize)]
    struct NewWindowReq {
        action: &'static str,
    }
    let value = ipc_call("action", NewWindowReq { action: "new_window" }).await?;

    let window_id = value
        .get("window_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| GhosttyError::ParseError("missing window_id in new_window response".into()))?;

    Ok(window_id)
}

/// Close a Ghostty window.
pub async fn close_window(window_id: u64) -> Result<(), GhosttyError> {
    #[derive(Serialize)]
    struct CloseReq {
        action: &'static str,
        window_id: u64,
    }
    ipc_call(
        "action",
        CloseReq {
            action: "close_window",
            window_id,
        },
    )
    .await?;
    Ok(())
}

/// Focus a Ghostty window (bring it to front).
pub async fn focus_window(window_id: u64) -> Result<(), GhosttyError> {
    #[derive(Serialize)]
    struct FocusReq {
        action: &'static str,
        window_id: u64,
    }
    ipc_call(
        "action",
        FocusReq {
            action: "focus_window",
            window_id,
        },
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Input injection
// ---------------------------------------------------------------------------

/// The backend to use for injecting keystrokes / text.
#[derive(Debug, Clone, PartialEq)]
pub enum InputBackend {
    /// `xdotool type` / `xdotool key` (Linux X11)
    Xdotool,
    /// `osascript` System Events keystroke (macOS)
    Osascript,
    /// No-op (useful for testing or headless envs)
    None,
}

impl InputBackend {
    pub fn detect() -> Self {
        if let Ok(val) = env::var("GHOSTTY_INPUT_BACKEND") {
            return match val.to_lowercase().as_str() {
                "xdotool" => Self::Xdotool,
                "osascript" => Self::Osascript,
                "none" => Self::None,
                _ => Self::detect_auto(),
            };
        }
        Self::detect_auto()
    }

    fn detect_auto() -> Self {
        #[cfg(target_os = "macos")]
        return Self::Osascript;

        // Linux: probe xdotool
        if std::process::Command::new("xdotool")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Self::Xdotool;
        }

        Self::None
    }
}

/// Send literal text to the focused Ghostty window.
///
/// The caller is responsible for focusing the target window first.
pub async fn send_text(window_id: u64, text: &str) -> Result<(), GhosttyError> {
    focus_window(window_id).await?;
    // Small yield to let the WM process the focus event before we inject
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    match InputBackend::detect() {
        InputBackend::Xdotool => xdotool_type(text).await,
        InputBackend::Osascript => osascript_keystroke(text).await,
        InputBackend::None => Err(GhosttyError::InputBackend(
            "no input backend available; install xdotool or set GHOSTTY_INPUT_BACKEND".into(),
        )),
    }
}

/// Send a key name (e.g. `"ctrl+c"`, `"Return"`) to the focused window.
pub async fn send_key(window_id: u64, key: &str) -> Result<(), GhosttyError> {
    focus_window(window_id).await?;
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    match InputBackend::detect() {
        InputBackend::Xdotool => xdotool_key(key).await,
        InputBackend::Osascript => osascript_key(key).await,
        InputBackend::None => Err(GhosttyError::InputBackend(
            "no input backend available; install xdotool or set GHOSTTY_INPUT_BACKEND".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// xdotool helpers
// ---------------------------------------------------------------------------

async fn xdotool_type(text: &str) -> Result<(), GhosttyError> {
    let output = Command::new("xdotool")
        .args(["type", "--clearmodifiers", "--delay", "0", "--", text])
        .output()
        .await
        .map_err(GhosttyError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GhosttyError::InputBackend(format!(
            "xdotool type failed: {stderr}"
        )));
    }
    Ok(())
}

async fn xdotool_key(key: &str) -> Result<(), GhosttyError> {
    let output = Command::new("xdotool")
        .args(["key", "--clearmodifiers", "--", key])
        .output()
        .await
        .map_err(GhosttyError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GhosttyError::InputBackend(format!(
            "xdotool key failed: {stderr}"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// osascript helpers (macOS)
// ---------------------------------------------------------------------------

async fn osascript_keystroke(text: &str) -> Result<(), GhosttyError> {
    // Escape the string for AppleScript
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"tell application "System Events" to keystroke "{escaped}""#
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
        .map_err(GhosttyError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GhosttyError::InputBackend(format!(
            "osascript keystroke failed: {stderr}"
        )));
    }
    Ok(())
}

async fn osascript_key(key: &str) -> Result<(), GhosttyError> {
    // Map wrightty key names to osascript key code names
    let key_code = map_key_to_applescript(key);
    let script = format!(
        r#"tell application "System Events" to key code {key_code}"#
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
        .map_err(GhosttyError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GhosttyError::InputBackend(format!(
            "osascript key code failed: {stderr}"
        )));
    }
    Ok(())
}

/// Map a key name (xdotool-style) to an AppleScript key code number.
fn map_key_to_applescript(key: &str) -> &'static str {
    match key {
        "Return" | "return" | "KP_Return" => "36",
        "Tab" | "tab" => "48",
        "BackSpace" | "backspace" => "51",
        "Delete" | "delete" => "117",
        "Escape" | "escape" => "53",
        "Up" | "KP_Up" => "126",
        "Down" | "KP_Down" => "125",
        "Left" | "KP_Left" => "123",
        "Right" | "KP_Right" => "124",
        "Home" => "115",
        "End" => "119",
        "Page_Up" => "116",
        "Page_Down" => "121",
        "F1" => "122",
        "F2" => "120",
        "F3" => "99",
        "F4" => "118",
        "F5" => "96",
        "F6" => "97",
        "F7" => "98",
        "F8" => "100",
        "F9" => "101",
        "F10" => "109",
        "F11" => "103",
        "F12" => "111",
        _ => "36", // fall back to Return
    }
}
