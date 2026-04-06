use std::net::{SocketAddr, TcpListener};
use std::process;
use std::time::Duration;

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_bridge_zellij::rpc::build_rpc_module;
use wrightty_bridge_zellij::zellij;

const PORT_RANGE_START: u16 = 9481;
const PORT_RANGE_END: u16 = 9500;

#[derive(Parser)]
#[command(
    name = "wrightty-bridge-zellij",
    about = "Bridge that translates wrightty protocol calls into zellij CLI action commands"
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on. If not specified, auto-selects the next available port starting at 9481.
    #[arg(long)]
    port: Option<u16>,

    /// Interval in seconds to check if zellij is still running. 0 to disable.
    #[arg(long, default_value_t = 10)]
    watchdog_interval: u64,
}

fn find_available_port(host: &str, start: u16, end: u16) -> Option<u16> {
    for port in start..=end {
        if TcpListener::bind(format!("{host}:{port}")).is_ok() {
            return Some(port);
        }
    }
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("wrightty_bridge_zellij=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    // --- Startup health check ---
    tracing::info!("Checking zellij connectivity...");
    match zellij::health_check().await {
        Ok(()) => {
            let session = zellij::session_name().unwrap_or_else(|_| "unknown".to_string());
            tracing::info!("zellij is reachable (session: {session})");
        },
        Err(e) => {
            eprintln!("error: Cannot connect to zellij: {e}");
            eprintln!();
            eprintln!("This bridge must run from within a zellij session.");
            eprintln!("Start zellij first:");
            eprintln!("  zellij");
            eprintln!("Then run this bridge from within the session.");
            process::exit(1);
        },
    }

    let port = match cli.port {
        Some(p) => p,
        None => find_available_port(&cli.host, PORT_RANGE_START, PORT_RANGE_END)
            .ok_or_else(|| anyhow::anyhow!("No available port in range {PORT_RANGE_START}-{PORT_RANGE_END}"))?,
    };

    let addr: SocketAddr = format!("{}:{}", cli.host, port).parse()?;

    let module = build_rpc_module(None, None)?;

    let server = Server::builder().build(addr).await?;

    let handle = server.start(module);

    let session = zellij::session_name().unwrap_or_else(|_| "unknown".to_string());
    tracing::info!("wrightty-bridge-zellij listening on ws://{addr} (session: {session})");
    println!("wrightty-bridge-zellij listening on ws://{addr}");

    // --- Watchdog: periodically check zellij is still alive ---
    if cli.watchdog_interval > 0 {
        let interval = Duration::from_secs(cli.watchdog_interval);
        let server_handle = handle.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            loop {
                tokio::time::sleep(interval).await;
                match zellij::health_check().await {
                    Ok(()) => {
                        if consecutive_failures > 0 {
                            tracing::info!("zellij reconnected");
                            consecutive_failures = 0;
                        }
                    },
                    Err(e) => {
                        consecutive_failures += 1;
                        tracing::warn!(
                            "zellij health check failed ({consecutive_failures}): {e}"
                        );
                        if consecutive_failures >= 3 {
                            tracing::error!(
                                "zellij unreachable after {consecutive_failures} checks, shutting down"
                            );
                            server_handle.stop().unwrap();
                            return;
                        }
                    },
                }
            }
        });
    }

    handle.stopped().await;

    Ok(())
}
