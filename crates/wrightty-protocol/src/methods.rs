use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::*;

// --- Wrightty domain ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInfoResult {
    #[serde(flatten)]
    pub info: ServerInfo,
}

// --- Authentication ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticateParams {
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticateResult {
    pub authenticated: bool,
}

// --- Session domain ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

fn default_cols() -> u16 {
    80
}
fn default_rows() -> u16 {
    24
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateResult {
    pub session_id: SessionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDestroyParams {
    pub session_id: SessionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDestroyResult {
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListResult {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGetInfoParams {
    pub session_id: SessionId,
}

// --- Input domain ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSendKeysParams {
    pub session_id: SessionId,
    pub keys: Vec<KeyInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSendTextParams {
    pub session_id: SessionId,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSendMouseParams {
    pub session_id: SessionId,
    pub event: String,
    pub button: String,
    pub row: u32,
    pub col: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
}

// --- Screen domain ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenGetContentsParams {
    pub session_id: SessionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<ScreenRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenGetContentsResult {
    pub rows: u32,
    pub cols: u32,
    pub cursor: CursorState,
    pub cells: Vec<Vec<CellData>>,
    pub alternate_screen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenGetTextParams {
    pub session_id: SessionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<ScreenRegion>,
    #[serde(default = "default_true")]
    pub trim_trailing_whitespace: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenGetTextResult {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenGetScrollbackParams {
    pub session_id: SessionId,
    #[serde(default = "default_scrollback_lines")]
    pub lines: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_scrollback_lines() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollbackLine {
    pub text: String,
    pub line_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenGetScrollbackResult {
    pub lines: Vec<ScrollbackLine>,
    pub total_scrollback: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenScreenshotParams {
    pub session_id: SessionId,
    pub format: ScreenshotFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<FontConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    pub family: String,
    pub size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenScreenshotResult {
    pub format: ScreenshotFormat,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenWaitForTextParams {
    pub session_id: SessionId,
    pub pattern: String,
    #[serde(default)]
    pub is_regex: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<ScreenRegion>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_timeout() -> u64 {
    30000
}
fn default_interval() -> u64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMatch {
    pub text: String,
    pub row: u32,
    pub col: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenWaitForTextResult {
    pub found: bool,
    pub matches: Vec<TextMatch>,
    pub elapsed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenWaitForCursorParams {
    pub session_id: SessionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenWaitForCursorResult {
    pub cursor: CursorState,
    pub elapsed: u64,
}

// --- Terminal domain ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResizeParams {
    pub session_id: SessionId,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalGetSizeParams {
    pub session_id: SessionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalGetSizeResult {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSetColorPaletteParams {
    pub session_id: SessionId,
    pub palette: ColorPalette,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalGetModesParams {
    pub session_id: SessionId,
}

// --- Events domain ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsSubscribeParams {
    pub session_id: SessionId,
    pub events: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<EventsSubscribeOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsSubscribeOptions {
    #[serde(default = "default_debounce")]
    pub screen_debounce_ms: u64,
}

fn default_debounce() -> u64 {
    16
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsSubscribeResult {
    pub subscription_id: String,
    pub subscribed_events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsUnsubscribeParams {
    pub subscription_id: String,
}
