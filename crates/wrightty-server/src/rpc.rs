use std::time::{Duration, Instant};
use std::sync::Arc;

use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::RpcModule;

use wrightty_core::input;
use wrightty_protocol::error;
use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

use crate::state::{AppState, VideoFrame, VideoRecording};

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

    // --- Screen.getContents ---
    module.register_method("Screen.getContents", |params, state, _| {
        let p: ScreenGetContentsParams = params.parse()?;
        let mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        let data = session.get_contents();
        serde_json::to_value(ScreenGetContentsResult {
            rows: data.rows,
            cols: data.cols,
            cursor: data.cursor,
            cells: data.cells,
            alternate_screen: data.alternate_screen,
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
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

    // --- Screen.getScrollback ---
    module.register_method("Screen.getScrollback", |params, state, _| {
        let p: ScreenGetScrollbackParams = params.parse()?;
        let mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        let (lines, total_scrollback) = session.get_scrollback(p.lines, p.offset);
        serde_json::to_value(ScreenGetScrollbackResult {
            lines,
            total_scrollback,
        })
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Screen.screenshot ---
    module.register_method("Screen.screenshot", |params, state, _| {
        let p: ScreenScreenshotParams = params.parse()?;
        let mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;

        match p.format {
            ScreenshotFormat::Text => {
                let text = session.get_text();
                serde_json::to_value(ScreenScreenshotResult {
                    format: ScreenshotFormat::Text,
                    data: text,
                    width: None,
                    height: None,
                })
                .map_err(|e| proto_err(-32603, e.to_string()))
            }
            ScreenshotFormat::Json => {
                let data = session.get_contents();
                let json_data = serde_json::to_string(&data.cells)
                    .map_err(|e| proto_err(-32603, e.to_string()))?;
                serde_json::to_value(ScreenScreenshotResult {
                    format: ScreenshotFormat::Json,
                    data: json_data,
                    width: Some(data.cols),
                    height: Some(data.rows),
                })
                .map_err(|e| proto_err(-32603, e.to_string()))
            }
            _ => Err(proto_err(error::NOT_SUPPORTED, "screenshot format not supported")),
        }
    })?;

    // --- Screen.waitForText ---
    module.register_async_method("Screen.waitForText", |params, state, _| async move {
        let p: ScreenWaitForTextParams = params.parse()?;

        let deadline = Instant::now() + Duration::from_millis(p.timeout);
        let interval = Duration::from_millis(p.interval.max(10));

        let re = if p.is_regex {
            Some(
                regex::Regex::new(&p.pattern)
                    .map_err(|_| proto_err(error::INVALID_PATTERN, "invalid regex pattern"))?,
            )
        } else {
            None
        };

        loop {
            let text = {
                let mgr = state.session_manager.lock().unwrap();
                let session = mgr
                    .get(&p.session_id)
                    .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;
                session.get_text()
            };

            let matched = if let Some(ref re) = re {
                re.is_match(&text)
            } else {
                text.contains(&p.pattern)
            };

            if matched {
                let elapsed = deadline
                    .checked_duration_since(Instant::now())
                    .map(|remaining| p.timeout - remaining.as_millis() as u64)
                    .unwrap_or(p.timeout);

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

            if Instant::now() >= deadline {
                return serde_json::to_value(ScreenWaitForTextResult {
                    found: false,
                    matches: vec![],
                    elapsed: p.timeout,
                })
                .map_err(|e| proto_err(-32603, e.to_string()));
            }

            tokio::time::sleep(interval).await;
        }
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

    // --- Recording.captureScreen ---
    module.register_method("Recording.captureScreen", |params, state, _| {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P { session_id: String }
        let p: P = params.parse()?;
        let mgr = state.session_manager.lock().unwrap();
        let session = mgr
            .get(&p.session_id)
            .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;
        let text = session.get_text();
        serde_json::to_value(serde_json::json!({ "data": text, "format": "text" }))
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Recording.startVideo ---
    module.register_async_method("Recording.startVideo", |params, state, _| async move {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            #[serde(default = "default_interval")]
            interval_ms: u64,
        }
        fn default_interval() -> u64 { 500 }

        let p: P = params.parse()?;

        // Validate session exists and get dimensions
        let (cols, rows) = {
            let mgr = state.session_manager.lock().unwrap();
            let s = mgr.get(&p.session_id)
                .ok_or_else(|| proto_err(error::SESSION_NOT_FOUND, "session not found"))?;
            s.size()
        };

        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let rec_id = format!("vid-{}", COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));

        {
            let mut recs = state.video_recordings.lock().unwrap();
            recs.insert(rec_id.clone(), VideoRecording {
                session_id: p.session_id.clone(),
                cols,
                rows,
                started_at: Instant::now(),
                interval_ms: p.interval_ms,
                frames: Vec::new(),
                running: true,
            });
        }

        // Spawn background task to capture frames
        let recs_arc = Arc::clone(&state.video_recordings);
        let rec_id_bg = rec_id.clone();
        let session_id_bg = p.session_id.clone();
        let session_mgr = Arc::clone(&state.session_manager);
        let interval = Duration::from_millis(p.interval_ms);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;

                let still_running = {
                    let recs = recs_arc.lock().unwrap();
                    recs.get(&rec_id_bg).map(|r| r.running).unwrap_or(false)
                };
                if !still_running { break; }

                let frame_text = {
                    let mgr = session_mgr.lock().unwrap();
                    mgr.get(&session_id_bg).map(|s| s.get_text())
                };
                if let Some(text) = frame_text {
                    let mut recs = recs_arc.lock().unwrap();
                    if let Some(rec) = recs.get_mut(&rec_id_bg) {
                        let elapsed = rec.started_at.elapsed().as_secs_f64();
                        rec.frames.push(VideoFrame { elapsed_secs: elapsed, text });
                    }
                }
            }
        });

        serde_json::to_value(serde_json::json!({ "recordingId": rec_id }))
            .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    // --- Recording.stopVideo ---
    module.register_method("Recording.stopVideo", |params, state, _| {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P { recording_id: String }
        let p: P = params.parse()?;

        let rec = {
            let mut recs = state.video_recordings.lock().unwrap();
            recs.remove(&p.recording_id)
                .ok_or_else(|| proto_err(-32001, "recording not found"))?
        };

        // Serialise as asciicast v2
        let start_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut cast = String::new();
        cast.push_str(&format!(
            "{{\"version\":2,\"width\":{},\"height\":{},\"timestamp\":{},\"title\":\"wrightty video\"}}\n",
            rec.cols, rec.rows, start_ts
        ));

        for frame in &rec.frames {
            // Each frame: clear screen then write content
            let clear = "\\u001b[2J\\u001b[H";
            let mut data = frame.text.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\r\\n")
                .replace('\r', "\\r");
            cast.push_str(&format!(
                "[{:.6},\"o\",\"{}{}\"]\\n",
                frame.elapsed_secs, clear, data
            ));
        }
        // Remove the trailing escaped newline and fix
        let cast = cast.replace("\\n", "\n");

        serde_json::to_value(serde_json::json!({
            "data": cast,
            "format": "asciicast",
            "frameCount": rec.frames.len()
        }))
        .map_err(|e| proto_err(-32603, e.to_string()))
    })?;

    Ok(module)
}
