use std::net::{SocketAddr, TcpListener};
use std::process;
use std::time::Duration;

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_bridge_kitty::kitty;
use wrightty_bridge_kitty::rpc::build_rpc_module;

const PORT_RANGE_START: u16 = 9461;
const PORT_RANGE_END: u16 = 9480;

#[derive(Parser)]
#[command(
    name = "wrightty-bridge-kitty",
    about = "Bridge that translates wrightty protocol calls into kitty remote control commands"
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on. If not specified, auto-selects the next available port starting at 9461.
    #[arg(long)]
    port: Option<u16>,

    /// Interval in seconds to check if kitty is still running. 0 to disable.
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
                .add_directive("wrightty_bridge_kitty=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    // --- Startup health check ---
    tracing::info!("Checking kitty connectivity...");
    match kitty::health_check().await {
        Ok(()) => tracing::info!("kitty is reachable"),
        Err(e) => {
            eprintln!("error: Cannot connect to kitty: {e}");
            eprintln!();
            eprintln!("Make sure kitty is running with remote control enabled.");
            eprintln!("Add to kitty.conf:");
            eprintln!("  allow_remote_control yes");
            eprintln!("Or launch kitty with:");
            eprintln!("  kitty --listen-on unix:/tmp/kitty.sock");
            eprintln!("And set:");
            eprintln!("  KITTY_LISTEN_ON=unix:/tmp/kitty.sock");
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

    tracing::info!("wrightty-bridge-kitty listening on ws://{addr}");
    println!("wrightty-bridge-kitty listening on ws://{addr}");

    // --- Watchdog: periodically check kitty is still alive ---
    if cli.watchdog_interval > 0 {
        let interval = Duration::from_secs(cli.watchdog_interval);
        let server_handle = handle.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            loop {
                tokio::time::sleep(interval).await;
                match kitty::health_check().await {
                    Ok(()) => {
                        if consecutive_failures > 0 {
                            tracing::info!("kitty reconnected");
                            consecutive_failures = 0;
                        }
                    },
                    Err(e) => {
                        consecutive_failures += 1;
                        tracing::warn!(
                            "kitty health check failed ({consecutive_failures}): {e}"
                        );
                        if consecutive_failures >= 3 {
                            tracing::error!(
                                "kitty unreachable after {consecutive_failures} checks, shutting down"
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
