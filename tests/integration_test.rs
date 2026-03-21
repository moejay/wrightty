use std::collections::HashMap;
use std::time::Duration;

use wrightty_client::WrighttyClient;
use wrightty_protocol::methods::SessionCreateParams;
use wrightty_protocol::types::KeyInput;
use wrightty_server::rpc::build_rpc_module;
use wrightty_server::state::AppState;

/// Start the server on a random available port and return the URL.
async fn start_server() -> (String, jsonrpsee::server::ServerHandle) {
    let state = AppState::new(64);
    let module = build_rpc_module(state).unwrap();

    let server = jsonrpsee::server::Server::builder()
        .build("127.0.0.1:0")
        .await
        .unwrap();

    let addr = server.local_addr().unwrap();
    let handle = server.start(module);

    (format!("ws://{addr}"), handle)
}

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
    assert_eq!(info.version, "0.1.0");
    assert_eq!(info.implementation, "wrightty-server");
    assert!(info.capabilities.supports_resize);
    assert!(info.capabilities.supports_session_create);
}
