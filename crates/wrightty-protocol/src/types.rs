use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellAttrs {
    pub bold: bool,
    pub italic: bool,
    pub underline: UnderlineStyle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<Rgb>,
    pub strikethrough: bool,
    pub dim: bool,
    pub blink: bool,
    pub reverse: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnderlineStyle {
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellData {
    pub char: String,
    pub width: u8,
    pub fg: Rgb,
    pub bg: Rgb,
    pub attrs: CellAttrs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorState {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub shape: CursorShape,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRegion {
    pub top: u32,
    pub left: u32,
    pub bottom: u32,
    pub right: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub running: bool,
    pub alternate_screen: bool,
}

// --- Key input types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyInput {
    Shorthand(String),
    Structured(KeyEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyEvent {
    pub key: KeyType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyType {
    Char,
    Enter,
    Tab,
    Backspace,
    Delete,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

// --- Screenshot types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScreenshotFormat {
    Text,
    Ansi,
    Json,
    Svg,
    Png,
}

// --- Capability types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub screenshot: Vec<ScreenshotFormat>,
    pub max_sessions: u32,
    pub supports_resize: bool,
    pub supports_scrollback: bool,
    pub supports_mouse: bool,
    pub supports_session_create: bool,
    pub supports_color_palette: bool,
    pub supports_raw_output: bool,
    pub supports_shell_integration: bool,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub version: String,
    pub implementation: String,
    pub capabilities: Capabilities,
}

// --- Terminal mode types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalModes {
    pub cursor_key_mode: String,
    pub keypad_mode: String,
    pub alternate_screen: bool,
    pub bracketed_paste: bool,
    pub mouse_tracking: String,
    pub mouse_encoding: String,
    pub focus_reporting: bool,
    pub cursor_visible: bool,
    pub cursor_style: String,
    pub auto_wrap: bool,
    pub reverse_video: bool,
    pub origin_mode: bool,
    pub synchronized_output: bool,
}

// --- Color palette ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette(pub HashMap<String, Rgb>);
