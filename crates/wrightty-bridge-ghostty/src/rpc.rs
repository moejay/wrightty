//! jsonrpsee RPC module that maps wrightty protocol methods to Ghostty IPC.

use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::RpcModule;

use wrightty_protocol::error;
use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

use crate::ghostty;

fn proto_err(code: i32, msg: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code, msg.into(), None::<()>)
}

fn not_supported(method: &str) -> ErrorObjectOwned {
    proto_err(
        error::NOT_SUPPORTED,
        format!("{method} is not supported by the ghostty bridge"),
    )
}

/// Parse a session ID string into a Ghostty window ID (u64).
fn parse_window_id(session_id: &str) -> Result<u64, ErrorObjectOwned> {
    session_id
        .parse::<u64>()
        .map_err(|_| proto_err(error::SESSION_NOT_FOUND, format!("invalid session id: {session_id}")))
}

/// Convert a wrightty `KeyInput` to an xdotool-compatible key name.
///
/// xdotool key names reference:
/// <https://gitlab.com/cunidev/gestures/-/wikis/xdotool-list-of-key-codes>
fn encode_key_to_xdotool(key: &KeyInput) -> String {
    match key {
        KeyInput::Shorthand(s) => shorthand_to_xdotool(s),
        KeyInput::Structured(event) => key_event_to_xdotool(event),
    }
}

fn shorthand_to_xdotool(s: &str) -> String {
    // Handle modifier combos like "ctrl+c", "alt+shift+f"
    if s.contains('+') {
        // xdotool uses "ctrl+c" style, just lowercase it
        return s
            .split('+')
            .map(normalize_key_name)
            .collect::<Vec<_>>()
            .join("+");
    }
    normalize_key_name(s)
}

fn normalize_key_name(name: &str) -> String {
    match name {
        "Enter" | "Return" => "Return".to_string(),
        "Tab" => "Tab".to_string(),
        "Backspace" => "BackSpace".to_string(),
        "Delete" => "Delete".to_string(),
        "Escape" | "Esc" => "Escape".to_string(),
        "ArrowUp" | "Up" => "Up".to_string(),
        "ArrowDown" | "Down" => "Down".to_string(),
        "ArrowRight" | "Right" => "Right".to_string(),
        "ArrowLeft" | "Left" => "Left".to_string(),
        "Home" => "Home".to_string(),
        "End" => "End".to_string(),
        "PageUp" => "Page_Up".to_string(),
        "PageDown" => "Page_Down".to_string(),
        "Insert" => "Insert".to_string(),
        "ctrl" | "Ctrl" | "control" | "Control" => "ctrl".to_string(),
        "alt" | "Alt" => "alt".to_string(),
        "shift" | "Shift" => "shift".to_string(),
        "super" | "Super" | "meta" | "Meta" => "super".to_string(),
        _ => name.to_lowercase(),
    }
}

fn key_event_to_xdotool(event: &KeyEvent) -> String {
    let has_ctrl = event.modifiers.iter().any(|m| matches!(m, Modifier::Ctrl));
    let has_alt = event.modifiers.iter().any(|m| matches!(m, Modifier::Alt));
    let has_shift = event.modifiers.iter().any(|m| matches!(m, Modifier::Shift));

    let base = match &event.key {
        KeyType::Char => event
            .char
            .as_deref()
            .map(normalize_key_name)
            .unwrap_or_default(),
        KeyType::Enter => "Return".to_string(),
        KeyType::Tab => "Tab".to_string(),
        KeyType::Backspace => "BackSpace".to_string(),
        KeyType::Delete => "Delete".to_string(),
        KeyType::Escape => "Escape".to_string(),
        KeyType::ArrowUp => "Up".to_string(),
        KeyType::ArrowDown => "Down".to_string(),
        KeyType::ArrowRight => "Right".to_string(),
        KeyType::ArrowLeft => "Left".to_string(),
        KeyType::Home => "Home".to_string(),
        KeyType::End => "End".to_string(),
        KeyType::PageUp => "Page_Up".to_string(),
        KeyType::PageDown => "Page_Down".to_string(),
        KeyType::Insert => "Insert".to_string(),
        KeyType::F => format!("F{}", event.n.unwrap_or(1)),
    };

    let mut parts: Vec<&str> = vec![];
    if has_ctrl {
        parts.push("ctrl");
    }
    if has_alt {
        parts.push("alt");
    }
    if has_shift {
        parts.push("shift");
    }

    if parts.is_empty() {
        base
    } else {
        format!("{}+{base}", parts.join("+"))
    }
}

pub fn build_rpc_module() -> anyhow::Result<RpcModule<()>> {
    let mut module = RpcModule::new(());

    // --- Wrightty.getInfo ---
    module.register_async_method("Wrightty.getInfo", |_params, _state, _| async move {
        serde_json::to_value(GetInfoResult {
            info: ServerInfo {
                version: "0.1.0".to_string(),
                implementation: "wrightty-bridge-ghostty".to_string(),
                capabilities: Capabilities {
                    screenshot: vec![],
                    max_sessions: 128,
                    supports_resize: false,
                    supports_scrollback: false,
                    supports_mouse: false,
                    supports_session_create: true,
                    supports_color_palette: false,
                    supports_raw_output: false,
                    supports_shell_integration: false,
                    events: vec![],
                },
            },
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.create ---
    module.register_async_method("Session.create", |_params, _state, _| async move {
        let window_id = ghostty::new_window()
            .await
            .map_err(|e| proto_err(error::SPAWN_FAILED, e.to_string()))?;

        serde_json::to_value(SessionCreateResult {
            session_id: window_id.to_string(),
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.destroy ---
    module.register_async_method("Session.destroy", |params, _state, _| async move {
        let p: SessionDestroyParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        ghostty::close_window(window_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        serde_json::to_value(SessionDestroyResult { exit_code: None })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.list ---
    module.register_async_method("Session.list", |_params, _state, _| async move {
        let windows = ghostty::list_windows()
            .await
            .map_err(|e| proto_err(-32603, e.to_string()))?;

        let sessions: Vec<SessionInfo> = windows
            .into_iter()
            .map(|w| SessionInfo {
                session_id: w.id.to_string(),
                title: w.title,
                cwd: w.cwd,
                cols: w.cols,
                rows: w.rows,
                pid: w.pid,
                running: true,
                alternate_screen: false,
            })
            .collect();

        serde_json::to_value(SessionListResult { sessions })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.getInfo ---
    module.register_async_method("Session.getInfo", |params, _state, _| async move {
        let p: SessionGetInfoParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        let w = ghostty::find_window(window_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        let info = SessionInfo {
            session_id: w.id.to_string(),
            title: w.title,
            cwd: w.cwd,
            cols: w.cols,
            rows: w.rows,
            pid: w.pid,
            running: true,
            alternate_screen: false,
        };

        serde_json::to_value(info).map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Input.sendText ---
    module.register_async_method("Input.sendText", |params, _state, _| async move {
        let p: InputSendTextParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        ghostty::send_text(window_id, &p.text)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Input.sendKeys ---
    module.register_async_method("Input.sendKeys", |params, _state, _| async move {
        let p: InputSendKeysParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        for key in &p.keys {
            // Single literal characters go via send_text for better compatibility
            let is_literal_char =
                matches!(key, KeyInput::Shorthand(s) if s.len() == 1 && !s.contains('+'));

            if is_literal_char {
                let text = match key {
                    KeyInput::Shorthand(s) => s.clone(),
                    _ => unreachable!(),
                };
                ghostty::send_text(window_id, &text)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
            } else {
                let key_str = encode_key_to_xdotool(key);
                ghostty::send_key(window_id, &key_str)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
            }
        }

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Terminal.getSize ---
    module.register_async_method("Terminal.getSize", |params, _state, _| async move {
        let p: TerminalGetSizeParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        let w = ghostty::find_window(window_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        serde_json::to_value(TerminalGetSizeResult {
            cols: w.cols,
            rows: w.rows,
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Unsupported methods ---

    module.register_async_method("Screen.getText", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported(
            "Screen.getText — Ghostty does not expose a screen-dump IPC; \
             use wrightty-server with ghostty-native support instead",
        ))
    })?;

    module.register_async_method("Screen.getContents", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported("Screen.getContents"))
    })?;

    module.register_async_method("Screen.screenshot", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported("Screen.screenshot"))
    })?;

    module.register_async_method("Terminal.resize", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported(
            "Terminal.resize — Ghostty window sizing is not exposed via IPC",
        ))
    })?;

    module.register_async_method("Input.sendMouse", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported("Input.sendMouse"))
    })?;

    Ok(module)
}
