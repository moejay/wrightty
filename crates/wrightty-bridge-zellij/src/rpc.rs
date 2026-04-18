//! jsonrpsee RPC module that maps wrightty protocol methods to zellij CLI action commands.
//!
//! Note: Zellij CLI actions operate on the focused/current pane within the active session.
//! The session_id parameter is accepted but currently used only for validation (must match
//! the active ZELLIJ_SESSION_NAME). Multi-pane targeting is not available via the CLI
//! action interface without a WASM plugin.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::RpcModule;

use wrightty_protocol::error;
use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

use crate::zellij;

fn proto_err(code: i32, msg: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code, msg.into(), None::<()>)
}

fn not_supported(method: &str) -> ErrorObjectOwned {
    proto_err(error::NOT_SUPPORTED, format!("{method} is not supported by the zellij bridge"))
}

/// Encode a wrightty KeyInput to the text/bytes it represents.
///
/// For most keys we use write-chars with escape sequences. For byte-level control,
/// we use zellij action write with raw bytes.
fn encode_key_to_escape(key: &KeyInput) -> String {
    match key {
        KeyInput::Shorthand(s) => shorthand_to_escape(s),
        KeyInput::Structured(event) => key_event_to_escape(event),
    }
}

fn shorthand_to_escape(s: &str) -> String {
    if let Some((modifier, key)) = s.split_once('+') {
        match modifier {
            "Ctrl" => {
                if key.len() == 1 {
                    let ch = key.chars().next().unwrap();
                    let ctrl = (ch.to_ascii_uppercase() as u8).wrapping_sub(b'@');
                    return (ctrl as char).to_string();
                }
                if let Some(seq) = named_key_escape(key) {
                    return seq.to_string();
                }
            }
            "Alt" => {
                let mut out = "\x1b".to_string();
                if key.len() == 1 {
                    out.push_str(key);
                } else if let Some(seq) = named_key_escape(key) {
                    out.push_str(seq);
                }
                return out;
            }
            _ => {}
        }
    }

    if let Some(seq) = named_key_escape(s) {
        return seq.to_string();
    }

    s.to_string()
}

fn named_key_escape(name: &str) -> Option<&'static str> {
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

fn key_event_to_escape(event: &KeyEvent) -> String {
    let has_ctrl = event.modifiers.iter().any(|m| matches!(m, Modifier::Ctrl));
    let has_alt = event.modifiers.iter().any(|m| matches!(m, Modifier::Alt));

    if has_alt {
        let inner = key_event_to_escape(&KeyEvent {
            key: event.key.clone(),
            modifiers: event.modifiers.iter().filter(|m| !matches!(m, Modifier::Alt)).cloned().collect(),
            char: event.char.clone(),
            n: event.n,
        });
        return format!("\x1b{inner}");
    }

    match &event.key {
        KeyType::Char => {
            let ch = event.char.as_deref().unwrap_or("");
            if has_ctrl && ch.len() == 1 {
                let c = ch.chars().next().unwrap();
                let ctrl = (c.to_ascii_uppercase() as u8).wrapping_sub(b'@');
                return (ctrl as char).to_string();
            }
            ch.to_string()
        }
        KeyType::Enter => "\r".to_string(),
        KeyType::Tab => "\t".to_string(),
        KeyType::Backspace => "\x7f".to_string(),
        KeyType::Delete => "\x1b[3~".to_string(),
        KeyType::Escape => "\x1b".to_string(),
        KeyType::ArrowUp => "\x1b[A".to_string(),
        KeyType::ArrowDown => "\x1b[B".to_string(),
        KeyType::ArrowRight => "\x1b[C".to_string(),
        KeyType::ArrowLeft => "\x1b[D".to_string(),
        KeyType::Home => "\x1b[H".to_string(),
        KeyType::End => "\x1b[F".to_string(),
        KeyType::PageUp => "\x1b[5~".to_string(),
        KeyType::PageDown => "\x1b[6~".to_string(),
        KeyType::Insert => "\x1b[2~".to_string(),
        KeyType::F => {
            let n = event.n.unwrap_or(1);
            match n {
                1 => "\x1bOP".to_string(),
                2 => "\x1bOQ".to_string(),
                3 => "\x1bOR".to_string(),
                4 => "\x1bOS".to_string(),
                5 => "\x1b[15~".to_string(),
                6 => "\x1b[17~".to_string(),
                7 => "\x1b[18~".to_string(),
                8 => "\x1b[19~".to_string(),
                9 => "\x1b[20~".to_string(),
                10 => "\x1b[21~".to_string(),
                11 => "\x1b[23~".to_string(),
                12 => "\x1b[24~".to_string(),
                _ => String::new(),
            }
        }
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
                let session_name = zellij::session_name().unwrap_or_else(|_| "unknown".to_string());
                serde_json::to_value(GetInfoResult {
                    info: ServerInfo {
                        version: "0.1.0".to_string(),
                        implementation: format!("wrightty-bridge-zellij@{session_name}"),
                        name: name_for_info.clone(),
                        authentication: if password_for_info.is_some() {
                            AuthenticationMode::Password
                        } else {
                            AuthenticationMode::None
                        },
                        capabilities: Capabilities {
                            screenshot: vec![ScreenshotFormat::Text],
                            max_sessions: 1,
                            supports_resize: false,
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

                zellij::new_pane()
                    .await
                    .map_err(|e| proto_err(error::SPAWN_FAILED, e.to_string()))?;

                let session_name = zellij::session_name()
                    .map_err(|e| proto_err(error::SPAWN_FAILED, e.to_string()))?;

                serde_json::to_value(SessionCreateResult { session_id: session_name })
                    .map_err(|e| proto_err(-32603, e.to_string()))
            }
        })?;
    }

    // --- Session.destroy ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Session.destroy", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                zellij::close_pane()
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

                let sessions = zellij::list_sessions()
                    .await
                    .map_err(|e| proto_err(-32603, e.to_string()))?;

                let session_infos: Vec<SessionInfo> = sessions
                    .into_iter()
                    .map(|s| SessionInfo {
                        session_id: s.name.clone(),
                        title: s.name,
                        cwd: None,
                        cols: 80,
                        rows: 24,
                        pid: None,
                        running: true,
                        alternate_screen: false,
                    })
                    .collect();

                serde_json::to_value(SessionListResult { sessions: session_infos })
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
                let session_name = zellij::session_name()
                    .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

                if p.session_id != session_name {
                    return Err(proto_err(error::SESSION_NOT_FOUND,
                        format!("session {} not found (current: {})", p.session_id, session_name)));
                }

                let info = SessionInfo {
                    session_id: session_name.clone(),
                    title: session_name,
                    cwd: None,
                    cols: 80,
                    rows: 24,
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

                zellij::write_chars(&p.text)
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

                for key in &p.keys {
                    let escape_seq = encode_key_to_escape(key);
                    // Use write-chars for printable text; for sequences with escape codes use write (bytes)
                    if escape_seq.is_ascii() && !escape_seq.contains('\x1b') && !escape_seq.contains('\r')
                        && !escape_seq.contains('\t') && !escape_seq.contains('\x7f')
                    {
                        zellij::write_chars(&escape_seq)
                            .await
                            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
                    } else {
                        let bytes = zellij::key_to_bytes(&escape_seq);
                        zellij::write_bytes(&bytes)
                            .await
                            .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;
                    }
                }

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

                let mut text = zellij::dump_screen()
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

    // --- Screen.waitForText ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Screen.waitForText", move |params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }

                let p: ScreenWaitForTextParams = params.parse()?;

                let re = if p.is_regex {
                    Some(
                        regex::Regex::new(&p.pattern)
                            .map_err(|_| proto_err(error::INVALID_PATTERN, "invalid regex pattern"))?,
                    )
                } else {
                    None
                };

                let start = std::time::Instant::now();
                let deadline = start + std::time::Duration::from_millis(p.timeout);
                let interval = std::time::Duration::from_millis(p.interval.max(10));

                loop {
                    let text = zellij::dump_screen()
                        .await
                        .map_err(|e| proto_err(error::SESSION_NOT_FOUND, e.to_string()))?;

                    let matched = if let Some(ref re) = re {
                        re.is_match(&text)
                    } else {
                        text.contains(&p.pattern)
                    };

                    if matched {
                        let elapsed = start.elapsed().as_millis() as u64;
                        let matches = if let Some(ref re) = re {
                            re.find_iter(&text)
                                .map(|m| TextMatch {
                                    text: m.as_str().to_string(),
                                    row: 0,
                                    col: 0,
                                    length: m.len() as u32,
                                })
                                .collect()
                        } else {
                            text.match_indices(p.pattern.as_str())
                                .map(|(_, s)| TextMatch {
                                    text: s.to_string(),
                                    row: 0,
                                    col: 0,
                                    length: s.len() as u32,
                                })
                                .collect()
                        };

                        return serde_json::to_value(ScreenWaitForTextResult {
                            found: true,
                            matches,
                            elapsed,
                        })
                        .map_err(|e| proto_err(-32603, e.to_string()));
                    }

                    if std::time::Instant::now() >= deadline {
                        return serde_json::to_value(ScreenWaitForTextResult {
                            found: false,
                            matches: vec![],
                            elapsed: p.timeout,
                        })
                        .map_err(|e| proto_err(-32603, e.to_string()));
                    }

                    tokio::time::sleep(interval).await;
                }
            }
        })?;
    }

    // --- Terminal.getSize (not supported) ---
    {
        let password = password.clone();
        let authenticated = Arc::clone(&authenticated);
        module.register_async_method("Terminal.getSize", move |_params, _state, ext| {
            let password = password.clone();
            let authenticated = Arc::clone(&authenticated);
            async move {
                if password.is_some() {
                    let conn_id = ext.get::<jsonrpsee::server::ConnectionId>().map(|c| c.0).unwrap_or(0);
                    if !authenticated.lock().unwrap().contains(&conn_id) {
                        return Err(proto_err(error::NOT_AUTHENTICATED, "not authenticated"));
                    }
                }
                Err::<serde_json::Value, _>(not_supported("Terminal.getSize"))
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
