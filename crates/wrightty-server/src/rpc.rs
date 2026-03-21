use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::RpcModule;

use wrightty_core::input;
use wrightty_protocol::error;
use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

use crate::state::AppState;

fn proto_err(code: i32, msg: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code, msg.into(), None::<()>)
}

pub fn build_rpc_module(state: AppState) -> anyhow::Result<RpcModule<AppState>> {
    let mut module = RpcModule::new(state);

    // --- Wrightty.getInfo ---
    module.register_method("Wrightty.getInfo", |_params, _state, _| {
        serde_json::to_value(GetInfoResult {
            info: ServerInfo {
                version: "0.1.0".to_string(),
                implementation: "wrightty-server".to_string(),
                capabilities: Capabilities {
                    screenshot: vec![ScreenshotFormat::Text, ScreenshotFormat::Json],
                    max_sessions: 64,
                    supports_resize: true,
                    supports_scrollback: true,
                    supports_mouse: false,
                    supports_session_create: true,
                    supports_color_palette: false,
                    supports_raw_output: true,
                    supports_shell_integration: false,
                    events: vec![
                        "Screen.updated".to_string(),
                        "Session.exited".to_string(),
                        "Terminal.bell".to_string(),
                        "Terminal.titleChanged".to_string(),
                    ],
                },
            },
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.create ---
    module.register_method("Session.create", |params, state, _| {
        let p: SessionCreateParams = params.parse()?;
        let mut mgr = state.session_manager.lock().unwrap();
        let id = mgr
            .create(p.shell, p.args, p.cols, p.rows, p.env, p.cwd)
            .map_err(|e| proto_err(error::SPAWN_FAILED, e.to_string()))?;
        serde_json::to_value(SessionCreateResult { session_id: id })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.destroy ---
    module.register_method("Session.destroy", |params, state, _| {
        let p: SessionDestroyParams = params.parse()?;
        let mut mgr = state.session_manager.lock().unwrap();
        mgr.destroy(&p.session_id)
            .map_err(|_| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;
        serde_json::to_value(SessionDestroyResult { exit_code: None })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.list ---
    module.register_method("Session.list", |_params, state, _| {
        let mgr = state.session_manager.lock().unwrap();
        let sessions = mgr.list();
        serde_json::to_value(SessionListResult { sessions })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Session.getInfo ---
    module.register_method("Session.getInfo", |params, state, _| {
        let p: SessionGetInfoParams = params.parse()?;
        let mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;
        let (cols, rows) = session.size();
        let info = SessionInfo {
            session_id: session.id.clone(),
            title: session.title.clone(),
            cwd: None,
            cols,
            rows,
            pid: None,
            running: session.is_running(),
            alternate_screen: false,
        };
        serde_json::to_value(info).map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Input.sendKeys ---
    module.register_method("Input.sendKeys", |params, state, _| {
        let p: InputSendKeysParams = params.parse()?;
        let mut mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get_mut(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        let bytes = input::encode_keys(&p.keys);
        session
            .write_bytes(&bytes)
            .map_err(|e| proto_err(-32603, e.to_string()))?;

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Input.sendText ---
    module.register_method("Input.sendText", |params, state, _| {
        let p: InputSendTextParams = params.parse()?;
        let mut mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get_mut(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        session
            .write_bytes(p.text.as_bytes())
            .map_err(|e| proto_err(-32603, e.to_string()))?;

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Screen.getText ---
    module.register_method("Screen.getText", |params, state, _| {
        let p: ScreenGetTextParams = params.parse()?;
        let mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        let text = session.get_text();
        serde_json::to_value(ScreenGetTextResult { text })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Terminal.resize ---
    module.register_method("Terminal.resize", |params, state, _| {
        let p: TerminalResizeParams = params.parse()?;
        let mut mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get_mut(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        session
            .resize(p.cols, p.rows)
            .map_err(|e| proto_err(-32603, e.to_string()))?;

        Ok::<_, ErrorObjectOwned>(serde_json::json!({}))
    })?;

    // --- Terminal.getSize ---
    module.register_method("Terminal.getSize", |params, state, _| {
        let p: TerminalGetSizeParams = params.parse()?;
        let mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        let (cols, rows) = session.size();
        serde_json::to_value(TerminalGetSizeResult { cols, rows })
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    Ok(module)
}
