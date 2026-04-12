//! Raw WebSocket JSON-RPC client — bypasses Sec-WebSocket-Accept validation.
//! Used as a fallback for servers with non-standard WebSocket handshakes.

use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

pub struct RawWsClient {
    stream: Mutex<TcpStream>,
    next_id: AtomicU64,
}

impl RawWsClient {
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let url = url.strip_prefix("ws://").ok_or("URL must start with ws://")?;
        let (host, port) = parse_host_port(url)?;

        let mut stream = TcpStream::connect(format!("{host}:{port}")).await?;

        // WebSocket upgrade — don't validate Sec-WebSocket-Accept
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let req = format!(
            "GET / HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: Upgrade\r\n\
             Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: {key}\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).await?;

        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await?;
        let resp = std::str::from_utf8(&buf[..n])?;
        if !resp.contains("101") {
            return Err(format!("WebSocket upgrade failed: {resp}").into());
        }

        Ok(Self {
            stream: Mutex::new(stream),
            next_id: AtomicU64::new(1),
        })
    }

    pub async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let msg = serde_json::to_string(&req)?;

        let mut stream = self.stream.lock().await;

        // Send masked WebSocket text frame
        write_ws_frame(&mut stream, msg.as_bytes()).await?;

        // Read response frame
        let payload = read_ws_frame(&mut stream).await?;
        let resp: serde_json::Value = serde_json::from_str(&payload)?;

        if let Some(err) = resp.get("error") {
            return Err(format!("RPC error: {err}").into());
        }

        let result = resp
            .get("result")
            .ok_or("Missing 'result' in JSON-RPC response")?;
        Ok(serde_json::from_value(result.clone())?)
    }
}

fn parse_host_port(url: &str) -> Result<(String, u16), Box<dyn std::error::Error>> {
    // url is "host:port" or "host:port/path" after stripping ws://
    let addr = url.split('/').next().unwrap_or(url);
    let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid address: {addr}").into());
    }
    let port: u16 = parts[0].parse()?;
    let host = parts[1].to_string();
    Ok((host, port))
}

async fn write_ws_frame(
    stream: &mut TcpStream,
    payload: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mask: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
    let mut frame = vec![0x81u8]; // FIN + text opcode
    if payload.len() < 126 {
        frame.push(0x80 | payload.len() as u8);
    } else if payload.len() <= 65535 {
        frame.push(0x80 | 126);
        frame.push((payload.len() >> 8) as u8);
        frame.push(payload.len() as u8);
    } else {
        frame.push(0x80 | 127);
        for i in (0..8).rev() {
            frame.push((payload.len() >> (i * 8)) as u8);
        }
    }
    frame.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    stream.write_all(&frame).await?;
    Ok(())
}

async fn read_ws_frame(
    stream: &mut TcpStream,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; 65536];
    let mut total = 0;

    // Read at least 2 bytes (frame header)
    while total < 2 {
        let n = stream.read(&mut buf[total..]).await?;
        if n == 0 {
            return Err("Connection closed".into());
        }
        total += n;
    }

    let masked = (buf[1] & 0x80) != 0;
    let mut payload_len = (buf[1] & 0x7F) as usize;
    let mut offset = 2;

    if payload_len == 126 {
        while total < 4 {
            let n = stream.read(&mut buf[total..]).await?;
            if n == 0 {
                return Err("Connection closed".into());
            }
            total += n;
        }
        payload_len = ((buf[2] as usize) << 8) | buf[3] as usize;
        offset = 4;
    } else if payload_len == 127 {
        while total < 10 {
            let n = stream.read(&mut buf[total..]).await?;
            if n == 0 {
                return Err("Connection closed".into());
            }
            total += n;
        }
        payload_len = 0;
        for i in 0..8 {
            payload_len = (payload_len << 8) | buf[2 + i] as usize;
        }
        offset = 10;
    }

    let mask_key = if masked {
        let mk_offset = offset;
        offset += 4;
        Some([
            buf[mk_offset],
            buf[mk_offset + 1],
            buf[mk_offset + 2],
            buf[mk_offset + 3],
        ])
    } else {
        None
    };

    // Read until we have the full payload
    let needed = offset + payload_len;
    if needed > buf.len() {
        buf.resize(needed, 0);
    }
    while total < needed {
        let n = stream.read(&mut buf[total..]).await?;
        if n == 0 {
            return Err("Connection closed".into());
        }
        total += n;
    }

    let mut payload = buf[offset..offset + payload_len].to_vec();
    if let Some(mask) = mask_key {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
    }

    String::from_utf8(payload).map_err(|e| e.into())
}
