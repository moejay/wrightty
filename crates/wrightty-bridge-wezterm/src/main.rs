use std::net::{SocketAddr, TcpListener};
use std::process;
use std::time::Duration;

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_bridge_wezterm::rpc::build_rpc_module;
use wrightty_bridge_wezterm::wezterm;

const PORT_RANGE_START: u16 = 9420;
const PORT_RANGE_END: u16 = 9440;

#[derive(Parser)]
#[command(
    name = "wrightty-bridge-wezterm",
    about = "Bridge that translates wrightty protocol calls into wezterm cli commands"
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on. If not specified, auto-selects the next available port starting at 9420.
    #[arg(long)]
    port: Option<u16>,

    /// Interval in seconds to check if WezTerm is still running. 0 to disable.
    #[arg(long, default_value_t = 5)]
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
                .add_directive("wrightty_bridge_wezterm=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    // --- Startup health check ---
    tracing::info!("Checking WezTerm connectivity...");
    match wezterm::health_check().await {
        Ok(()) => tracing::info!("WezTerm is reachable"),
        Err(e) => {
            eprintln!("error: Cannot connect to WezTerm: {e}");
            eprintln!();
            eprintln!("Make sure WezTerm is running. If using flatpak, set:");
            eprintln!("  WEZTERM_CMD=\"flatpak run --command=wezterm org.wezfurlong.wezterm\"");
            process::exit(1);
        },
    }

    let port = match cli.port {
        Some(p) => p,
        None => find_available_port(&cli.host, PORT_RANGE_START, PORT_RANGE_END)
            .ok_or_else(|| anyhow::anyhow!("No available port in range {PORT_RANGE_START}-{PORT_RANGE_END}"))?,
    };

    let addr: SocketAddr = format!("{}:{}", cli.host, port).parse()?;

    let module = build_rpc_module()?;

    let server = Server::builder().build(addr).await?;

    let handle = server.start(module);

    tracing::info!("wrightty-bridge-wezterm listening on ws://{addr}");
    println!("wrightty-bridge-wezterm listening on ws://{addr}");

    // --- Watchdog: periodically check WezTerm is still alive ---
    if cli.watchdog_interval > 0 {
        let interval = Duration::from_secs(cli.watchdog_interval);
        let server_handle = handle.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            loop {
                tokio::time::sleep(interval).await;
                match wezterm::health_check().await {
                    Ok(()) => {
                        if consecutive_failures > 0 {
                            tracing::info!("WezTerm reconnected");
                            consecutive_failures = 0;
                        }
                    },
                    Err(e) => {
                        consecutive_failures += 1;
                        tracing::warn!(
                            "WezTerm health check failed ({consecutive_failures}): {e}"
                        );
                        if consecutive_failures >= 3 {
                            tracing::error!(
                                "WezTerm unreachable after {consecutive_failures} checks, shutting down"
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
