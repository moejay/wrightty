//! Shared server infrastructure for all wrightty server modes.

use std::future::Future;
use std::net::{SocketAddr, TcpListener};
use std::time::Duration;

use jsonrpsee::server::Server;
use jsonrpsee::RpcModule;

/// Port range for wrightty servers (covers all modes).
pub const PORT_RANGE_START: u16 = 9420;
pub const PORT_RANGE_END: u16 = 9520;

/// Find the next available port in a range.
pub fn find_available_port(host: &str, start: u16, end: u16) -> Option<u16> {
    for port in start..=end {
        if TcpListener::bind(format!("{host}:{port}")).is_ok() {
            return Some(port);
        }
    }
    None
}

/// Start a JSON-RPC WebSocket server and block until it stops.
pub async fn start_server<S: Clone + Send + Sync + 'static>(
    host: &str,
    port: u16,
    name: &str,
    module: RpcModule<S>,
) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let server = Server::builder().build(addr).await?;
    let handle = server.start(module);

    tracing::info!("{name} listening on ws://{addr}");
    println!("{name} listening on ws://{addr}");

    handle.stopped().await;
    Ok(())
}

/// Start a server with a watchdog that periodically checks health.
///
/// If `health_check` fails 3 consecutive times, the server shuts down.
pub async fn start_server_with_watchdog<S, F, Fut>(
    host: &str,
    port: u16,
    name: &str,
    module: RpcModule<S>,
    watchdog_interval: u64,
    health_check: F,
) -> anyhow::Result<()>
where
    S: Clone + Send + Sync + 'static,
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), Box<dyn std::error::Error>>> + Send,
{
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let server = Server::builder().build(addr).await?;
    let handle = server.start(module);

    tracing::info!("{name} listening on ws://{addr}");
    println!("{name} listening on ws://{addr}");

    if watchdog_interval > 0 {
        let interval = Duration::from_secs(watchdog_interval);
        let server_handle = handle.clone();
        let bridge_name = name.to_string();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            loop {
                tokio::time::sleep(interval).await;
                match health_check().await {
                    Ok(()) => {
                        if consecutive_failures > 0 {
                            tracing::info!("{bridge_name} reconnected");
                            consecutive_failures = 0;
                        }
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        tracing::warn!(
                            "{bridge_name} health check failed ({consecutive_failures}): {e}"
                        );
                        if consecutive_failures >= 3 {
                            tracing::error!(
                                "{bridge_name} unreachable after {consecutive_failures} checks, shutting down"
                            );
                            server_handle.stop().unwrap();
                            return;
                        }
                    }
                }
            }
        });
    }

    handle.stopped().await;
    Ok(())
}
