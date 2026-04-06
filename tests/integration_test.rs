use std::collections::HashMap;
use std::time::Duration;

use wrightty_client::WrighttyClient;
use wrightty_protocol::methods::SessionCreateParams;
use wrightty_protocol::types::{AuthenticationMode, KeyInput, ScreenshotFormat};
use wrightty_server::rpc::build_rpc_module;
use wrightty_server::state::AppState;

/// Start the server on a random available port and return the URL.
async fn start_server() -> (String, jsonrpsee::server::ServerHandle) {
    let state = AppState::new(64, None, None);
    let module = build_rpc_module(state).unwrap();

    let server = jsonrpsee::server::Server::builder()
        .build("127.0.0.1:0")
        .await
        .unwrap();

    let addr = server.local_addr().unwrap();
    let handle = server.start(module);

    (format!("ws://{addr}"), handle)
}

async fn start_server_with_auth(name: Option<String>, password: Option<String>) -> (String, jsonrpsee::server::ServerHandle) {
    let state = AppState::new(64, name, password);
    let module = build_rpc_module(state).unwrap();
    let server = jsonrpsee::server::Server::builder()
        .build("127.0.0.1:0")
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let handle = server.start(module);
    (format!("ws://{addr}"), handle)
}

fn default_session() -> SessionCreateParams {
    SessionCreateParams {
        shell: Some("/bin/sh".to_string()),
        args: vec![],
        cols: 80,
        rows: 24,
        env: HashMap::new(),
        cwd: None,
    }
}

// ─── Existing tests (kept) ───────────────────────────────────────────────────

#[tokio::test]
async fn test_create_session_and_read_screen() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    // Create a session
    let session_id = client
        .session_create(SessionCreateParams {
            shell: Some("/bin/sh".to_string()),
            args: vec![],
            cols: 80,
            rows: 24,
            env: HashMap::new(),
            cwd: None,
        })
        .await
        .unwrap();

    // Give the shell a moment to start and print its prompt
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send "echo hello" + Enter
    client
        .send_text(&session_id, "echo hello\n")
        .await
        .unwrap();

    // Wait for output to be processed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Read the screen
    let text = client.get_text(&session_id).await.unwrap();
    println!("Screen text:\n---\n{text}\n---");

    assert!(
        text.contains("hello"),
        "Expected 'hello' in screen output, got:\n{text}"
    );

    // Clean up
    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_send_keys() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client
        .session_create(SessionCreateParams {
            shell: Some("/bin/sh".to_string()),
            args: vec![],
            cols: 80,
            rows: 24,
            env: HashMap::new(),
            cwd: None,
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send keystrokes using the shorthand format
    let keys: Vec<KeyInput> = vec![
        KeyInput::Shorthand("e".to_string()),
        KeyInput::Shorthand("c".to_string()),
        KeyInput::Shorthand("h".to_string()),
        KeyInput::Shorthand("o".to_string()),
        KeyInput::Shorthand(" ".to_string()),
        KeyInput::Shorthand("w".to_string()),
        KeyInput::Shorthand("o".to_string()),
        KeyInput::Shorthand("r".to_string()),
        KeyInput::Shorthand("l".to_string()),
        KeyInput::Shorthand("d".to_string()),
        KeyInput::Shorthand("Enter".to_string()),
    ];

    client.send_keys(&session_id, keys).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let text = client.get_text(&session_id).await.unwrap();
    println!("Screen text:\n---\n{text}\n---");

    assert!(
        text.contains("world"),
        "Expected 'world' in screen output, got:\n{text}"
    );

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_resize() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client
        .session_create(SessionCreateParams {
            shell: Some("/bin/sh".to_string()),
            args: vec![],
            cols: 80,
            rows: 24,
            env: HashMap::new(),
            cwd: None,
        })
        .await
        .unwrap();

    // Check initial size
    let (cols, rows) = client.get_size(&session_id).await.unwrap();
    assert_eq!(cols, 80);
    assert_eq!(rows, 24);

    // Resize
    client.resize(&session_id, 120, 40).await.unwrap();

    // Verify new size
    let (cols, rows) = client.get_size(&session_id).await.unwrap();
    assert_eq!(cols, 120);
    assert_eq!(rows, 40);

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_session_list() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    // No sessions initially
    let sessions = client.session_list().await.unwrap();
    assert!(sessions.is_empty());

    // Create two sessions
    let id1 = client
        .session_create(SessionCreateParams {
            shell: Some("/bin/sh".to_string()),
            args: vec![],
            cols: 80,
            rows: 24,
            env: HashMap::new(),
            cwd: None,
        })
        .await
        .unwrap();

    let id2 = client
        .session_create(SessionCreateParams {
            shell: Some("/bin/sh".to_string()),
            args: vec![],
            cols: 80,
            rows: 24,
            env: HashMap::new(),
            cwd: None,
        })
        .await
        .unwrap();

    let sessions = client.session_list().await.unwrap();
    assert_eq!(sessions.len(), 2);

    // Destroy one
    client.session_destroy(&id1).await.unwrap();

    let sessions = client.session_list().await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, id2);

    client.session_destroy(&id2).await.unwrap();
}

#[tokio::test]
async fn test_get_info() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let info = client.get_info().await.unwrap();
    assert!(!info.version.is_empty());
    assert_eq!(info.implementation, "wrightty-server");
    assert!(matches!(info.authentication, AuthenticationMode::None));
    assert!(info.capabilities.supports_resize);
    assert!(info.capabilities.supports_session_create);
}

// ─── Screen domain ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_screen_get_contents_dimensions() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let contents = client.get_contents(&session_id).await.unwrap();

    // Screen dimensions match what we requested
    assert_eq!(contents.cols, 80);
    assert_eq!(contents.rows, 24);

    // Cell grid has the right shape
    assert_eq!(contents.cells.len(), 24, "expected 24 rows");
    for row in &contents.cells {
        assert_eq!(row.len(), 80, "expected 80 cols per row");
    }

    // Cursor is within bounds
    assert!(contents.cursor.row < 24);
    assert!(contents.cursor.col < 80);

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_get_contents_cell_data() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    client.send_text(&session_id, "echo hi\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let contents = client.get_contents(&session_id).await.unwrap();

    // Flatten all chars and find "h","i" somewhere
    let all_chars: String = contents.cells.iter().flat_map(|row| row.iter().map(|c| c.char.as_str())).collect();
    assert!(all_chars.contains('h'), "expected 'h' in cell data");
    assert!(all_chars.contains('i'), "expected 'i' in cell data");

    // Each cell has valid width (0, 1, or 2)
    for row in &contents.cells {
        for cell in row {
            assert!(cell.width <= 2, "unexpected cell width: {}", cell.width);
        }
    }

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_get_scrollback_empty_initially() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Fresh terminal has no scrollback
    let result = client.get_scrollback(&session_id, 100, 0).await.unwrap();
    assert_eq!(result.total_scrollback, 0);
    assert!(result.lines.is_empty());

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_get_scrollback_after_fill() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    // Small terminal so it scrolls quickly
    let session_id = client
        .session_create(SessionCreateParams {
            shell: Some("/bin/sh".to_string()),
            args: vec![],
            cols: 80,
            rows: 5,
            env: HashMap::new(),
            cwd: None,
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Print more lines than the terminal height so scrollback fills up
    for i in 0..10 {
        client
            .send_text(&session_id, &format!("echo line{i}\n"))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    let result = client.get_scrollback(&session_id, 100, 0).await.unwrap();
    println!("Scrollback lines: {}", result.total_scrollback);
    println!("Returned: {:?}", result.lines.iter().map(|l| &l.text).collect::<Vec<_>>());

    // We should have some scrollback now
    assert!(result.total_scrollback > 0, "expected scrollback after filling terminal");
    // Line numbers in scrollback are negative
    for line in &result.lines {
        assert!(line.line_number < 0, "scrollback line_number should be negative, got {}", line.line_number);
    }

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_screenshot_text_format() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    client.send_text(&session_id, "echo screenshot_test\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let result = client.screenshot(&session_id, ScreenshotFormat::Text).await.unwrap();

    assert!(
        matches!(result.format, ScreenshotFormat::Text),
        "format should be Text"
    );
    assert!(
        result.data.contains("screenshot_test"),
        "screenshot data should contain the echoed text, got: {}",
        result.data
    );
    // Text format has no pixel dimensions
    assert!(result.width.is_none());
    assert!(result.height.is_none());

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_screenshot_json_format() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let result = client.screenshot(&session_id, ScreenshotFormat::Json).await.unwrap();

    assert!(matches!(result.format, ScreenshotFormat::Json));
    assert_eq!(result.width, Some(80));
    assert_eq!(result.height, Some(24));
    // data is valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&result.data).expect("JSON screenshot data should be valid JSON");
    assert!(parsed.is_array(), "JSON screenshot should be an array of rows");

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_wait_for_text_success() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send text in the background, then wait for it
    client.send_text(&session_id, "echo wait_target\n").await.unwrap();

    let result = client
        .wait_for_text(&session_id, "wait_target", false, 5000)
        .await
        .unwrap();

    assert!(result.found, "wait_for_text should have found 'wait_target'");
    assert!(!result.matches.is_empty());
    assert!(result.elapsed < 5000);

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_wait_for_text_timeout() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Wait for text that will never appear
    let result = client
        .wait_for_text(&session_id, "this_text_will_never_appear_xyz", false, 300)
        .await
        .unwrap();

    assert!(!result.found, "wait_for_text should have timed out");
    assert!(result.matches.is_empty());

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_screen_wait_for_text_regex() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    client.send_text(&session_id, "echo regex123\n").await.unwrap();

    // Regex: match "regex" followed by digits
    let result = client
        .wait_for_text(&session_id, r"regex\d+", true, 5000)
        .await
        .unwrap();

    assert!(result.found, "regex wait should have matched");
    assert!(!result.matches.is_empty());
    assert!(result.matches[0].text.starts_with("regex"));

    client.session_destroy(&session_id).await.unwrap();
}

// ─── Input domain ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_input_send_text_multiple_commands() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send several commands
    client.send_text(&session_id, "echo first\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.send_text(&session_id, "echo second\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.send_text(&session_id, "echo third\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let text = client.get_text(&session_id).await.unwrap();
    println!("Screen:\n{text}");

    assert!(text.contains("first") || text.contains("second") || text.contains("third"),
        "At least one output line should be visible");

    client.session_destroy(&session_id).await.unwrap();
}

#[tokio::test]
async fn test_input_send_keys_special_keys() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Type a partial command then backspace to erase it, then type correct command
    let keys: Vec<KeyInput> = vec![
        KeyInput::Shorthand("e".to_string()),
        KeyInput::Shorthand("c".to_string()),
        KeyInput::Shorthand("x".to_string()), // typo
        KeyInput::Shorthand("Backspace".to_string()),
        KeyInput::Shorthand("h".to_string()),
        KeyInput::Shorthand("o".to_string()),
        KeyInput::Shorthand(" ".to_string()),
        KeyInput::Shorthand("k".to_string()),
        KeyInput::Shorthand("e".to_string()),
        KeyInput::Shorthand("y".to_string()),
        KeyInput::Shorthand("s".to_string()),
        KeyInput::Shorthand("Enter".to_string()),
    ];

    client.send_keys(&session_id, keys).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let text = client.get_text(&session_id).await.unwrap();
    println!("Screen:\n{text}");

    assert!(text.contains("keys"), "Expected 'keys' in output, got:\n{text}");

    client.session_destroy(&session_id).await.unwrap();
}

// ─── Error cases ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_error_get_text_invalid_session() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let result = client.get_text("nonexistent-session-id").await;
    assert!(result.is_err(), "should return error for invalid session ID");
    let err = result.unwrap_err().to_string();
    println!("Error: {err}");
    assert!(
        err.contains("1001") || err.contains("session not found"),
        "expected SESSION_NOT_FOUND error code 1001, got: {err}"
    );
}

#[tokio::test]
async fn test_error_get_contents_invalid_session() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let result = client.get_contents("nonexistent-session-id").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("1001") || err.contains("session not found"),
        "expected SESSION_NOT_FOUND, got: {err}"
    );
}

#[tokio::test]
async fn test_error_get_scrollback_invalid_session() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let result = client.get_scrollback("nonexistent-session-id", 10, 0).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("1001") || err.contains("session not found"),
        "expected SESSION_NOT_FOUND, got: {err}"
    );
}

#[tokio::test]
async fn test_error_send_text_invalid_session() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let result = client.send_text("nonexistent-session-id", "hello\n").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("1001") || err.contains("session not found"),
        "expected SESSION_NOT_FOUND, got: {err}"
    );
}

#[tokio::test]
async fn test_error_send_keys_invalid_session() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let result = client
        .send_keys("nonexistent-session-id", vec![KeyInput::Shorthand("a".to_string())])
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("1001") || err.contains("session not found"),
        "expected SESSION_NOT_FOUND, got: {err}"
    );
}

#[tokio::test]
async fn test_error_destroy_nonexistent_session() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let result = client.session_destroy("nonexistent-session-id").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_error_screenshot_unsupported_format() {
    let (url, _handle) = start_server().await;
    let client = WrighttyClient::connect(&url).await.unwrap();

    let session_id = client.session_create(default_session()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // PNG/SVG not implemented — should return NOT_SUPPORTED error
    let result = client.screenshot(&session_id, ScreenshotFormat::Png).await;
    assert!(result.is_err(), "PNG screenshot should return an error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("1006") || err.contains("not supported"),
        "expected NOT_SUPPORTED error code 1006, got: {err}"
    );

    client.session_destroy(&session_id).await.unwrap();
}

// ─── Authentication tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_info_includes_name() {
    let (url, _handle) = start_server_with_auth(Some("test-server".to_string()), None).await;
    let client = WrighttyClient::connect(&url).await.unwrap();
    let info = client.get_info().await.unwrap();
    assert_eq!(info.name, Some("test-server".to_string()));
    assert!(matches!(info.authentication, AuthenticationMode::None));
}

#[tokio::test]
async fn test_get_info_shows_password_auth() {
    let (url, _handle) = start_server_with_auth(None, Some("secret".to_string())).await;
    let client = WrighttyClient::connect(&url).await.unwrap();
    let info = client.get_info().await.unwrap();
    assert!(matches!(info.authentication, AuthenticationMode::Password));
}

#[tokio::test]
async fn test_auth_blocks_unauthenticated() {
    let (url, _handle) = start_server_with_auth(None, Some("secret".to_string())).await;
    let client = WrighttyClient::connect(&url).await.unwrap();
    // Session.list should fail without auth
    let result = client.session_list().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_auth_success() {
    let (url, _handle) = start_server_with_auth(None, Some("secret".to_string())).await;
    let client = WrighttyClient::connect(&url).await.unwrap();
    // Authenticate
    client.authenticate("secret").await.unwrap();
    // Now session_list should work
    let sessions = client.session_list().await.unwrap();
    assert!(sessions.is_empty()); // no sessions created yet
}

#[tokio::test]
async fn test_auth_wrong_password() {
    let (url, _handle) = start_server_with_auth(None, Some("secret".to_string())).await;
    let client = WrighttyClient::connect(&url).await.unwrap();
    let result = client.authenticate("wrong").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_no_auth_required_works() {
    let (url, _handle) = start_server_with_auth(None, None).await;
    let client = WrighttyClient::connect(&url).await.unwrap();
    // Should work without authentication
    let sessions = client.session_list().await.unwrap();
    assert!(sessions.is_empty());
}
