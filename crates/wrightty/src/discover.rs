use clap::Args;

use crate::server::{PORT_RANGE_START, PORT_RANGE_END};

#[derive(Args)]
pub struct DiscoverArgs {
    /// Host to scan
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub async fn run(args: DiscoverArgs) -> anyhow::Result<()> {
    let found = scan_ports(&args.host).await;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&found)?);
        return Ok(());
    }

    if found.is_empty() {
        println!("No wrightty servers found on ports {PORT_RANGE_START}-{PORT_RANGE_END}.");
        println!();
        println!("Start one with:");
        println!("  wrightty term --headless");
        println!("  wrightty term --bridge-tmux");
        println!("  wrightty term --bridge-wezterm");
        return Ok(());
    }

    for s in &found {
        let name = s["name"].as_str().unwrap_or("");
        let auth = s["authentication"].as_str().unwrap_or("none");
        let auth_tag = if auth == "password" { " [password]" } else { "" };
        let name_tag = if name.is_empty() { String::new() } else { format!(" ({name})") };
        println!(
            "  {}  {} v{}{}{}",
            s["url"].as_str().unwrap_or(""),
            s["implementation"].as_str().unwrap_or(""),
            s["version"].as_str().unwrap_or(""),
            name_tag,
            auth_tag,
        );
    }

    Ok(())
}

/// Scan all ports in parallel and return discovered servers sorted by port.
pub async fn scan_ports(host: &str) -> Vec<serde_json::Value> {
    let mut handles = Vec::new();

    for port in PORT_RANGE_START..=PORT_RANGE_END {
        let url = format!("ws://{host}:{port}");
        handles.push(tokio::spawn(try_connect(url)));
    }

    let mut found = Vec::new();
    for handle in handles {
        if let Ok(Some(info)) = handle.await {
            found.push(info);
        }
    }
    found.sort_by_key(|v| v["url"].as_str().unwrap_or("").to_string());
    found
}

/// Find the first available server URL.
pub async fn discover_first(host: &str) -> Option<String> {
    let servers = scan_ports(host).await;
    servers.first().and_then(|s| s["url"].as_str().map(String::from))
}

async fn try_connect(url: String) -> Option<serde_json::Value> {
    // Try jsonrpsee first (strict RFC 6455 compliant)
    if let Some(result) = try_connect_jsonrpsee(&url).await {
        return Some(result);
    }
    // Fall back to raw socket probe for servers with non-standard WS handshake
    // (e.g. alacritty fork with incorrect Sec-WebSocket-Accept)
    try_connect_raw(&url).await
}

async fn try_connect_jsonrpsee(url: &str) -> Option<serde_json::Value> {
    use jsonrpsee::core::client::ClientT;
    use jsonrpsee::core::params::ObjectParams;
    use jsonrpsee::ws_client::WsClientBuilder;

    let client = WsClientBuilder::default()
        .connection_timeout(std::time::Duration::from_secs(2))
        .request_timeout(std::time::Duration::from_secs(2))
        .build(url)
        .await
        .ok()?;

    let info: serde_json::Value = client
        .request("Wrightty.getInfo", ObjectParams::new())
        .await
        .ok()?;

    Some(normalize_info(url, info))
}

/// Raw socket probe — bypasses WebSocket key validation.
/// Used as a fallback for servers that don't compute Sec-WebSocket-Accept correctly.
async fn try_connect_raw(url: &str) -> Option<serde_json::Value> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let port: u16 = url.rsplit(':').next()?.parse().ok()?;
    let host = "127.0.0.1";

    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        TcpStream::connect(format!("{host}:{port}")),
    )
    .await
    .ok()?
    .ok()?;

    // WebSocket upgrade (don't validate accept key)
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let req = format!(
        "GET / HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: Upgrade\r\n\
         Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: {key}\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await.ok()?;

    let mut buf = vec![0u8; 4096];
    let n = tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buf))
        .await
        .ok()?
        .ok()?;
    let resp = std::str::from_utf8(&buf[..n]).ok()?;
    if !resp.contains("101") {
        return None;
    }

    // Send masked JSON-RPC frame for Wrightty.getInfo
    let msg = r#"{"jsonrpc":"2.0","id":1,"method":"Wrightty.getInfo","params":{}}"#;
    let payload = msg.as_bytes();
    let mask: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
    let mut frame = vec![0x81u8]; // FIN + text
    if payload.len() < 126 {
        frame.push(0x80 | payload.len() as u8);
    } else {
        frame.push(0x80 | 126);
        frame.push((payload.len() >> 8) as u8);
        frame.push(payload.len() as u8);
    }
    frame.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    stream.write_all(&frame).await.ok()?;

    // Read response frame — server may send header and payload separately
    let mut buf = vec![0u8; 4096];
    let mut total = 0;

    // Read at least 2 bytes (frame header)
    while total < 2 {
        let n = tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buf[total..]))
            .await
            .ok()?
            .ok()?;
        if n == 0 { return None; }
        total += n;
    }

    let mut offset = 2;
    let mut payload_len = (buf[1] & 0x7F) as usize;
    if payload_len == 126 {
        while total < 4 {
            let n = tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buf[total..]))
                .await.ok()?.ok()?;
            if n == 0 { return None; }
            total += n;
        }
        payload_len = ((buf[2] as usize) << 8) | buf[3] as usize;
        offset = 4;
    }

    // Read until we have the full payload
    let needed = offset + payload_len;
    while total < needed {
        let n = tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buf[total..]))
            .await.ok()?.ok()?;
        if n == 0 { return None; }
        total += n;
    }

    let json_str = std::str::from_utf8(&buf[offset..offset + payload_len]).ok()?;
    let resp: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let info = resp.get("result")?.clone();

    Some(normalize_info(url, info))
}

/// Normalize getInfo response — handles both wrapped and unwrapped formats.
fn normalize_info(url: &str, info: serde_json::Value) -> serde_json::Value {
    let server_info = if info.get("info").is_some() {
        info["info"].clone()
    } else {
        info
    };

    let mut result = serde_json::json!({ "url": url });
    if let Some(obj) = server_info.as_object() {
        for (k, v) in obj {
            result[k] = v.clone();
        }
    }
    result
}
