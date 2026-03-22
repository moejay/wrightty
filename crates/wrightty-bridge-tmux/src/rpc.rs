//! jsonrpsee RPC module that maps wrightty protocol methods to tmux CLI commands.

use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::RpcModule;

use wrightty_protocol::error;
use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

use crate::tmux;

fn proto_err(code: i32, msg: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code, msg.into(), None::<()>)
}

fn not_supported(method: &str) -> ErrorObjectOwned {
    proto_err(error::NOT_SUPPORTED, format!("{method} is not supported by the tmux bridge"))
}

/// Encode wrightty KeyInput values into tmux key name strings.
///
/// tmux `send-keys` without `-l` interprets key names like `Enter`, `C-c`, `M-x`, `Up`, etc.
/// We use this for structured key events. For plain text we use `send-keys -l`.
fn encode_key_to_tmux(key: &KeyInput) -> String {
    match key {
        KeyInput::Shorthand(s) => shorthand_to_tmux(s),
        KeyInput::Structured(event) => key_event_to_tmux(event),
    }
}

fn shorthand_to_tmux(s: &str) -> String {
    // Modifier combos like "Ctrl+c" -> "C-c", "Alt+x" -> "M-x"
    if let Some((modifier, key)) = s.split_once('+') {
        match modifier {
            "Ctrl" => return format!("C-{}", key.to_lowercase()),
            "Alt" => return format!("M-{key}"),
            _ => {}
        }
    }

    // Named keys
    match s {
        "Enter" | "Return" => "Enter".to_string(),
        "Tab" => "Tab".to_string(),
        "Backspace" => "BSpace".to_string(),
        "Delete" => "Delete".to_string(),
        "Escape" | "Esc" => "Escape".to_string(),
        "ArrowUp" | "Up" => "Up".to_string(),
        "ArrowDown" | "Down" => "Down".to_string(),
        "ArrowRight" | "Right" => "Right".to_string(),
        "ArrowLeft" | "Left" => "Left".to_string(),
        "Home" => "Home".to_string(),
        "End" => "End".to_string(),
        "PageUp" => "PPage".to_string(),
        "PageDown" => "NPage".to_string(),
        "Insert" => "Insert".to_string(),
        _ => s.to_string(),
    }
}

fn key_event_to_tmux(event: &KeyEvent) -> String {
    let has_ctrl = event.modifiers.iter().any(|m| matches!(m, Modifier::Ctrl));
    let has_alt = event.modifiers.iter().any(|m| matches!(m, Modifier::Alt));

    let base = match &event.key {
        KeyType::Char => event.char.as_deref().unwrap_or("").to_string(),
        KeyType::Enter => "Enter".to_string(),
        KeyType::Tab => "Tab".to_string(),
        KeyType::Backspace => "BSpace".to_string(),
        KeyType::Delete => "Delete".to_string(),
        KeyType::Escape => "Escape".to_string(),
        KeyType::ArrowUp => "Up".to_string(),
        KeyType::ArrowDown => "Down".to_string(),
        KeyType::ArrowRight => "Right".to_string(),
        KeyType::ArrowLeft => "Left".to_string(),
        KeyType::Home => "Home".to_string(),
        KeyType::End => "End".to_string(),
        KeyType::PageUp => "PPage".to_string(),
        KeyType::PageDown => "NPage".to_string(),
        KeyType::Insert => "Insert".to_string(),
        KeyType::F => format!("F{}", event.n.unwrap_or(1)),
    };

    let mut result = base;
    if has_ctrl {
        result = format!("C-{result}");
    }
    if has_alt {
        result = format!("M-{result}");
    }
    result
}

pub fn build_rpc_module() -> anyhow::Result<RpcModule<()>> {
    let mut module = RpcModule::new(());

    // --- Wrightty.getInfo ---
    module.register_async_method("Wrightty.getInfo", |_params, _state, _| async move {
        serde_json::to_value(GetInfoResult {
            info: ServerInfo {
                version: "0.1.0".to_string(),
                implementation: "wrightty-bridge-tmux".to_string(),
                capabilities: Capabilities {
                    screenshot: vec![ScreenshotFormat::Text],
                    max_sessions: 512,
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
        let target = tmux::new_window(None)
            .await
            .map_err(|e| proto_err(error::SPAWN_FAILED, e.to_string()))?;

        serde_json::to_value(SessionCreateResult { session_id: target })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.destroy ---
    module.register_async_method("Session.destroy", |params, _state, _| async move {
        let p: SessionDestroyParams = params.parse()?;

        tmux::kill_pane(&p.session_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        serde_json::to_value(SessionDestroyResult { exit_code: None })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.list ---
    module.register_async_method("Session.list", |_params, _state, _| async move {
        let panes = tmux::list_panes()
            .await
            .map_err(|e| proto_err(-32603, e.to_string()))?;

        let sessions: Vec<SessionInfo> = panes
            .into_iter()
            .map(|p| SessionInfo {
                session_id: p.target,
                title: if p.title.is_empty() {
                    format!("{}:{}.{}", p.session_name, p.window_index, p.pane_index)
                } else {
                    p.title
                },
                cwd: None,
                cols: p.cols,
                rows: p.rows,
                pid: if p.pid > 0 { Some(p.pid) } else { None },
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

        let pane = tmux::find_pane(&p.session_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        let info = SessionInfo {
            session_id: pane.target,
            title: if pane.title.is_empty() {
                format!("{}:{}.{}", pane.session_name, pane.window_index, pane.pane_index)
            } else {
                pane.title
            },
            cwd: None,
            cols: pane.cols,
            rows: pane.rows,
            pid: if pane.pid > 0 { Some(pane.pid) } else { None },
            running: true,
            alternate_screen: false,
        };

        serde_json::to_value(info).map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Input.sendText ---
    module.register_async_method("Input.sendText", |params, _state, _| async move {
        let p: InputSendTextParams = params.parse()?;

        tmux::send_text(&p.session_id, &p.text)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Input.sendKeys ---
    module.register_async_method("Input.sendKeys", |params, _state, _| async move {
        let p: InputSendKeysParams = params.parse()?;

        for key in &p.keys {
            let tmux_key = encode_key_to_tmux(key);
            // Determine if this is literal text (single char shorthand) or a key name
            let is_literal = matches!(key, KeyInput::Shorthand(s) if s.len() == 1 && !s.contains('+'));
            if is_literal {
                tmux::send_text(&p.session_id, &tmux_key)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
            } else {
                tmux::send_key(&p.session_id, &tmux_key)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
            }
        }

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Screen.getText ---
    module.register_async_method("Screen.getText", |params, _state, _| async move {
        let p: ScreenGetTextParams = params.parse()?;

        let mut text = tmux::capture_pane(&p.session_id)
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

        let pane = tmux::find_pane(&p.session_id)
            .await
            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

        serde_json::to_value(TerminalGetSizeResult {
            cols: pane.cols,
            rows: pane.rows,
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Terminal.resize ---
    module.register_async_method("Terminal.resize", |params, _state, _| async move {
        let p: TerminalResizeParams = params.parse()?;

        tmux::resize_pane(&p.session_id, p.cols, p.rows)
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
