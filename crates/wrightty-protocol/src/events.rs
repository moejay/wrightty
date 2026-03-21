use serde::{Deserialize, Serialize};

use crate::types::{CursorShape, Rgb, ScreenRegion, SessionId};

/// Common event envelope sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub subscription_id: String,
    pub event: String,
    pub session_id: SessionId,
    pub timestamp: u64,
    pub data: serde_json::Value,
}

// --- Tier 1: Core events ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenUpdated {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty_region: Option<ScreenRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExited {
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOutput {
    pub data: String, // base64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBell {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTitleChanged {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCwdChanged {
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalAlternateScreen {
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCursorChanged {
    pub visible: bool,
    pub shape: CursorShape,
    pub blinking: bool,
}

// --- Tier 2: Shell integration ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellPromptStart {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommandStart {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOutputStart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCommandFinished {
    pub exit_code: i32,
}

// --- Tier 2: Notifications ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalNotification {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub urgency: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalClipboardSet {
    pub selection: String,
    pub text: String,
    pub base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalProgressChanged {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalModeChanged {
    pub mode: String,
    pub enabled: bool,
}

// --- Tier 3: Extended ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalFocusChanged {
    pub focused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalColorPaletteChanged {
    pub changes: std::collections::HashMap<String, Rgb>,
    pub source: String,
}
