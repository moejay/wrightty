//! jsonrpsee RPC module that maps wrightty protocol methods to wezterm cli commands.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::RpcModule;

use wrightty_protocol::error;
use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

use crate::wezterm;

fn proto_err(code: i32, msg: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code, msg.into(), None::<()>)
}

fn not_supported(method: &str) -> ErrorObjectOwned {
    proto_err(error::NOT_SUPPORTED, format!("{method} is not supported by the WezTerm bridge"))
}

/// Parse a wrightty session ID string into a WezTerm pane ID.
fn parse_pane_id(session_id: &str) -> Result<u64, ErrorObjectOwned> {
    session_id
        .parse::<u64>()
        .map_err(|_| proto_err(error::SESSION_NOT_FOUND, format!("invalid session id: {session_id}")))
}

/// Encode a list of KeyInput values into a string for `wezterm cli send-text`.
fn encode_keys_to_string(keys: &[KeyInput]) -> String {
    let mut out = String::new();
    for key in keys {
        match key {
            KeyInput::Shorthand(s) => encode_shorthand(s, &mut out),
            KeyInput::Structured(event) => encode_key_event(event, &mut out),
        }
    }
    out
}

fn encode_shorthand(s: &str, out: &mut String) {
    // Check for modifier combos like "Ctrl+c"
    if let Some((modifier_str, key_str)) = s.split_once('+') {
        match modifier_str {
            "Ctrl" => {
                if key_str.len() == 1 {
                    let ch = key_str.chars().next().unwrap();
                    // Ctrl+letter: map to control character
                    let ctrl = (ch.to_ascii_uppercase() as u8).wrapping_sub(b'@');
                    out.push(ctrl as char);
                    return;
                }
                if let Some(s) = named_key_str(key_str) {
                    out.push_str(s);
                    return;
                }
            }
            "Alt" => {
                out.push('\x1b');
                if key_str.len() == 1 {
                    out.push_str(key_str);
                } else if let Some(s) = named_key_str(key_str) {
                    out.push_str(s);
                }
                return;
            }
            _ => {}
        }
    }

    // Named keys
    if let Some(s) = named_key_str(s) {
        out.push_str(s);
        return;
    }

    // Literal text
    out.push_str(s);
}

fn encode_key_event(event: &KeyEvent, out: &mut String) {
    let has_ctrl = event.modifiers.iter().any(|m| matches!(m, Modifier::Ctrl));
    let has_alt = event.modifiers.iter().any(|m| matches!(m, Modifier::Alt));

    if has_alt {
        out.push('\x1b');
    }

    let base = match &event.key {
        KeyType::Char => {
            let ch = event.char.as_deref().unwrap_or("");
            if has_ctrl && ch.len() == 1 {
                let c = ch.chars().next().unwrap();
                let ctrl = (c.to_ascii_uppercase() as u8).wrapping_sub(b'@');
                out.push(ctrl as char);
                return;
            }
            ch
        }
        KeyType::Enter => "\r",
        KeyType::Tab => "\t",
        KeyType::Backspace => "\x7f",
        KeyType::Delete => "\x1b[3~",
        KeyType::Escape => "\x1b",
        KeyType::ArrowUp => "\x1b[A",
        KeyType::ArrowDown => "\x1b[B",
        KeyType::ArrowRight => "\x1b[C",
        KeyType::ArrowLeft => "\x1b[D",
        KeyType::Home => "\x1b[H",
        KeyType::End => "\x1b[F",
        KeyType::PageUp => "\x1b[5~",
        KeyType::PageDown => "\x1b[6~",
        KeyType::Insert => "\x1b[2~",
        KeyType::F => {
            let n = event.n.unwrap_or(1);
            match n {
                1 => "\x1bOP",
                2 => "\x1bOQ",
                3 => "\x1bOR",
                4 => "\x1bOS",
                5 => "\x1b[15~",
                6 => "\x1b[17~",
                7 => "\x1b[18~",
                8 => "\x1b[19~",
                9 => "\x1b[20~",
                10 => "\x1b[21~",
                11 => "\x1b[23~",
                12 => "\x1b[24~",
                _ => "",
            }
        }
    };
    out.push_str(base);
}

fn named_key_str(name: &str) -> Option<&'static str> {
    match name {
        "Enter" | "Return" => Some("\r"),
        "Tab" => Some("\t"),
        "Backspace" => Some("\x7f"),
        "Delete" => Some("\x1b[3~"),
        "Escape" | "Esc" => Some("\x1b"),
        "ArrowUp" | "Up" => Some("\x1b[A"),
        "ArrowDown" | "Down" => Some("\x1b[B"),
        "ArrowRight" | "Right" => Some("\x1b[C"),
        "ArrowLeft" | "Left" => Some("\x1b[D"),
        "Home" => Some("\x1b[H"),
        "End" => Some("\x1b[F"),
        "PageUp" => Some("\x1b[5~"),
        "PageDown" => Some("\x1b[6~"),
        "Insert" => Some("\x1b[2~"),
        _ => None,
    }
}

pub fn build_rpc_module(name: Option<String>, password: Option<String>) -> anyhow::Result<RpcModule<()>> {
    let mut module = RpcModule::new(());
    let authenticated: Arc<Mutex<HashSet<usize>>> = Arc::new(Mutex::new(HashSet::new()));

    // --- Wrightty.getInfo ---
    {
        let name_for_info = name.clone();
        let password_for_info = password.clone();
        module.register_async_method("Wrightty.getInfo", move |_params, _state, _| {
            let name_for_info = name_for_info.clone();
            let password_for_info = password_for_info.clone();
            async move {
                serde_json::to_value(GetInfoResult {
                    info: ServerInfo {
                        version: "0.1.0".to_string(),
                        implementation: "wrightty-bridge-wezterm".to_string(),
                        name: name_for_info.clone(),
                        authentication: if password_for_info.is_some() {
                            AuthenticationMode::Password
                        } else {
                            AuthenticationMode::None
                        },
                        capabilities: Capabilities {
                            screenshot: vec![ScreenshotFormat::Text],
                            max_sessions: 256,
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
            }
        })?;
    }

    // --- Wrightty.authenticate ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Wrightty.authenticate", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                let p: AuthenticateParams = params.parse()?;
                match &password {
                    Some(pw) if pw == &p.password => {
                        let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                        authenticated.lock().unwrap().insert(conn_id);
                        serde_json::to_value(AuthenticateResult { authenticated: true })
                            .map_err(|e| proto_err(-32603, e.to_string()))
                    }
                    Some(_) => Err(proto_err(error::AUTH_FAILED, "authentication failed")),
                    None => serde_json::to_value(AuthenticateResult { authenticated: true })
                        .map_err(|e| proto_err(-32603, e.to_string())),
                }
            }
        })?;
    }

    // --- Session.create ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Session.create", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let pane_id = wezterm::spawn_pane()
                    .await
                    .map_err(|e| proto_err(error::SPAWN_FAILED, e.to_string()))?;

                serde_json::to_value(SessionCreateResult {
                    session_id: pane_id.to_string(),
                })
                .map_err(|e| proto_err(-32603, e.to_string()))
            }
        })?;
    }

    // --- Session.destroy ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Session.destroy", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let p: SessionDestroyParams = params.parse()?;
                let pane_id = parse_pane_id(&p.session_id)?;

                wezterm::kill_pane(pane_id)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

                serde_json::to_value(SessionDestroyResult { exit_code: None })
                    .map_err(|e| proto_err(-32603, e.to_string()))
            }
        })?;
    }

    // --- Session.list ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Session.list", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let panes = wezterm::list_panes()
                    .await
                    .map_err(|e| proto_err(-32603, e.to_string()))?;

                let sessions: Vec<SessionInfo> = panes
                    .into_iter()
                    .map(|p| SessionInfo {
                        session_id: p.pane_id.to_string(),
                        title: p.title,
                        cwd: if p.cwd.is_empty() { None } else { Some(p.cwd) },
                        cols: p.size.cols,
                        rows: p.size.rows,
                        pid: None,
                        running: true,
                        alternate_screen: false,
                    })
                    .collect();

                serde_json::to_value(SessionListResult { sessions })
                    .map_err(|e| proto_err(-32603, e.to_string()))
            }
        })?;
    }

    // --- Session.getInfo ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Session.getInfo", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let p: SessionGetInfoParams = params.parse()?;
                let pane_id = parse_pane_id(&p.session_id)?;

                let pane = wezterm::find_pane(pane_id)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

                let info = SessionInfo {
                    session_id: pane.pane_id.to_string(),
                    title: pane.title,
                    cwd: if pane.cwd.is_empty() {
                        None
                    } else {
                        Some(pane.cwd)
                    },
                    cols: pane.size.cols,
                    rows: pane.size.rows,
                    pid: None,
                    running: true,
                    alternate_screen: false,
                };

                serde_json::to_value(info).map_err(|e| proto_err(-32603, e.to_string()))
            }
        })?;
    }

    // --- Input.sendText ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Input.sendText", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let p: InputSendTextParams = params.parse()?;
                let pane_id = parse_pane_id(&p.session_id)?;

                wezterm::send_text(pane_id, &p.text)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

                Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
            }
        })?;
    }

    // --- Input.sendKeys ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Input.sendKeys", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let p: InputSendKeysParams = params.parse()?;
                let pane_id = parse_pane_id(&p.session_id)?;

                let text = encode_keys_to_string(&p.keys);
                wezterm::send_text(pane_id, &text)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

                Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
            }
        })?;
    }

    // --- Screen.getText ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Screen.getText", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let p: ScreenGetTextParams = params.parse()?;
                let pane_id = parse_pane_id(&p.session_id)?;

                let mut text = wezterm::get_text(pane_id)
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
            }
        })?;
    }

    // --- Terminal.getSize ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Terminal.getSize", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let p: TerminalGetSizeParams = params.parse()?;
                let pane_id = parse_pane_id(&p.session_id)?;

                let pane = wezterm::find_pane(pane_id)
                    .await
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

                serde_json::to_value(TerminalGetSizeResult {
                    cols: pane.size.cols,
                    rows: pane.size.rows,
                })
                .map_err(|e| proto_err(-32603, e.to_string()))
            }
        })?;
    }

    // --- Terminal.resize (not supported) ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Terminal.resize", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }
                Err::<serde_json::Value, _>(not_supported("Terminal.resize"))
            }
        })?;
    }

    // --- Screen.getContents (not supported) ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Screen.getContents", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }
                Err::<serde_json::Value, _>(not_supported("Screen.getContents"))
            }
        })?;
    }

    // --- Screen.screenshot (not supported) ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Screen.screenshot", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }
                Err::<serde_json::Value, _>(not_supported("Screen.screenshot"))
            }
        })?;
    }

    // --- Input.sendMouse (not supported) ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Input.sendMouse", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }
                Err::<serde_json::Value, _>(not_supported("Input.sendMouse"))
            }
        })?;
    }

    Ok(module)
}
