//! jsonrpsee RPC module that maps wrightty protocol methods to kitty remote control commands.

use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::RpcModule;

use wrightty_protocol::error;
use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

use crate::kitty;

fn proto_err(code: i32, msg: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code, msg.into(), None::<()>)
}

fn not_supported(method: &str) -> ErrorObjectOwned {
    proto_err(error::NOT_SUPPORTED, format!("{method} is not supported by the kitty bridge"))
}

/// Parse a session ID string into a kitty window ID.
fn parse_window_id(session_id: &str) -> Result<u64, ErrorObjectOwned> {
    session_id
        .parse::<u64>()
        .map_err(|_| proto_err(error::SESSION_NOT_FOUND, format!("invalid session id: {session_id}")))
}

/// Convert a wrightty KeyInput to a kitty key name string.
///
/// kitty `send-key` uses its own key naming convention:
/// - Modifiers: `ctrl+c`, `alt+x`, `ctrl+shift+t`
/// - Named keys: `enter`, `tab`, `backspace`, `delete`, `escape`
/// - Arrows: `up`, `down`, `left`, `right`
/// - Function keys: `f1`..`f12`
fn encode_key_to_kitty(key: &KeyInput) -> String {
    match key {
        KeyInput::Shorthand(s) => shorthand_to_kitty(s),
        KeyInput::Structured(event) => key_event_to_kitty(event),
    }
}

fn shorthand_to_kitty(s: &str) -> String {
    if let Some((modifier, key)) = s.split_once('+') {
        let mod_lower = modifier.to_lowercase();
        return format!("{}+{}", mod_lower, key.to_lowercase());
    }

    match s {
        "Enter" | "Return" => "enter".to_string(),
        "Tab" => "tab".to_string(),
        "Backspace" => "backspace".to_string(),
        "Delete" => "delete".to_string(),
        "Escape" | "Esc" => "escape".to_string(),
        "ArrowUp" | "Up" => "up".to_string(),
        "ArrowDown" | "Down" => "down".to_string(),
        "ArrowRight" | "Right" => "right".to_string(),
        "ArrowLeft" | "Left" => "left".to_string(),
        "Home" => "home".to_string(),
        "End" => "end".to_string(),
        "PageUp" => "page_up".to_string(),
        "PageDown" => "page_down".to_string(),
        "Insert" => "insert".to_string(),
        _ => s.to_lowercase(),
    }
}

fn key_event_to_kitty(event: &KeyEvent) -> String {
    let has_ctrl = event.modifiers.iter().any(|m| matches!(m, Modifier::Ctrl));
    let has_alt = event.modifiers.iter().any(|m| matches!(m, Modifier::Alt));
    let has_shift = event.modifiers.iter().any(|m| matches!(m, Modifier::Shift));

    let base = match &event.key {
        KeyType::Char => event.char.as_deref().unwrap_or("").to_lowercase(),
        KeyType::Enter => "enter".to_string(),
        KeyType::Tab => "tab".to_string(),
        KeyType::Backspace => "backspace".to_string(),
        KeyType::Delete => "delete".to_string(),
        KeyType::Escape => "escape".to_string(),
        KeyType::ArrowUp => "up".to_string(),
        KeyType::ArrowDown => "down".to_string(),
        KeyType::ArrowRight => "right".to_string(),
        KeyType::ArrowLeft => "left".to_string(),
        KeyType::Home => "home".to_string(),
        KeyType::End => "end".to_string(),
        KeyType::PageUp => "page_up".to_string(),
        KeyType::PageDown => "page_down".to_string(),
        KeyType::Insert => "insert".to_string(),
        KeyType::F => format!("f{}", event.n.unwrap_or(1)),
    };

    let mut modifiers = Vec::new();
    if has_ctrl { modifiers.push("ctrl"); }
    if has_alt { modifiers.push("alt"); }
    if has_shift { modifiers.push("shift"); }

    if modifiers.is_empty() {
        base
    } else {
        format!("{}+{base}", modifiers.join("+"))
    }
}

pub fn build_rpc_module() -> anyhow::Result<RpcModule<()>> {
    let mut module = RpcModule::new(());

    // --- Wrightty.getInfo ---
    module.register_async_method("Wrightty.getInfo", |_params, _state, _| async move {
        serde_json::to_value(GetInfoResult {
            info: ServerInfo {
                version: "0.1.0".to_string(),
                implementation: "wrightty-bridge-kitty".to_string(),
                capabilities: Capabilities {
                    screenshot: vec![ScreenshotFormat::Text],
                    max_sessions: 256,
                    supports_resize: true,
                    supports_scrollback: true,
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
        let window_id = kitty::launch_window()
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

        kitty::close_window(window_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        serde_json::to_value(SessionDestroyResult { exit_code: None })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.list ---
    module.register_async_method("Session.list", |_params, _state, _| async move {
        let windows = kitty::list_windows()
            .await
            .map_err(|e| proto_err(-32603, e.to_string()))?;

        let sessions: Vec<SessionInfo> = windows
            .into_iter()
            .map(|w| SessionInfo {
                session_id: w.id.to_string(),
                title: w.title,
                cwd: w.cwd,
                cols: w.columns,
                rows: w.lines,
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

        let w = kitty::find_window(window_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        let info = SessionInfo {
            session_id: w.id.to_string(),
            title: w.title,
            cwd: w.cwd,
            cols: w.columns,
            rows: w.lines,
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

        kitty::send_text(window_id, &p.text)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Input.sendKeys ---
    module.register_async_method("Input.sendKeys", |params, _state, _| async move {
        let p: InputSendKeysParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        for key in &p.keys {
            // Plain single characters go via send-text; everything else via send-key
            let is_literal_char = matches!(key, KeyInput::Shorthand(s) if s.len() == 1 && !s.contains('+'));
            if is_literal_char {
                let text = match key {
                    KeyInput::Shorthand(s) => s.clone(),
                    _ => unreachable!(),
                };
                kitty::send_text(window_id, &text)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
            } else {
                let kitty_key = encode_key_to_kitty(key);
                kitty::send_key(window_id, &kitty_key)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
            }
        }

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Screen.getText ---
    module.register_async_method("Screen.getText", |params, _state, _| async move {
        let p: ScreenGetTextParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        let mut text = kitty::get_text(window_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        if p.trim_trailing_whitespace {
            text = text
                .lines()
                .map(|line| line.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
        }

        serde_json::to_value(ScreenGetTextResult { text })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Terminal.getSize ---
    module.register_async_method("Terminal.getSize", |params, _state, _| async move {
        let p: TerminalGetSizeParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        let w = kitty::find_window(window_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        serde_json::to_value(TerminalGetSizeResult {
            cols: w.columns,
            rows: w.lines,
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Terminal.resize ---
    module.register_async_method("Terminal.resize", |params, _state, _| async move {
        let p: TerminalResizeParams = params.parse()?;
        let window_id = parse_window_id(&p.session_id)?;

        kitty::resize_window(window_id, p.cols, p.rows)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Screen.getContents (not supported) ---
    module.register_async_method("Screen.getContents", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported("Screen.getContents"))
    })?;

    // --- Screen.screenshot (not supported) ---
    module.register_async_method("Screen.screenshot", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported("Screen.screenshot"))
    })?;

    // --- Input.sendMouse (not supported) ---
    module.register_async_method("Input.sendMouse", |_params, _state, _| async move {
        Err::<serde_json::Value, _>(not_supported("Input.sendMouse"))
    })?;

    Ok(module)
}
