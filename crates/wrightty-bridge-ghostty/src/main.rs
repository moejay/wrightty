use std::net::{SocketAddr, TcpListener};
use std::process;
use std::time::Duration;

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_bridge_ghostty::ghostty;
use wrightty_bridge_ghostty::rpc::build_rpc_module;

const PORT_RANGE_START: u16 = 9501;
const PORT_RANGE_END: u16 = 9520;

#[derive(Parser)]
#[command(
    name = "wrightty-bridge-ghostty",
    about = "Bridge that translates wrightty protocol calls into Ghostty IPC commands",
    long_about = "\
Connects to a running Ghostty terminal emulator via its Unix IPC socket and \
exposes the wrightty WebSocket JSON-RPC 2.0 interface.\n\n\
REQUIREMENTS:\n\
  - Ghostty must be running (the IPC socket is created on startup).\n\
  - For text/key injection, xdotool must be installed on Linux (X11) or\n\
    Accessibility must be enabled on macOS.\n\n\
ENVIRONMENT:\n\
  GHOSTTY_SOCKET          Override the IPC socket path.\n\
  GHOSTTY_INPUT_BACKEND   Force input backend: xdotool | osascript | none."
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on. If not specified, auto-selects the next available
    /// port starting at 9481.
    #[arg(long)]
    port: Option<u16>,

    /// Interval in seconds to check if Ghostty is still running. 0 to disable.
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
                .add_directive("wrightty_bridge_ghostty=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    // --- Startup health check ---
    tracing::info!("Checking Ghostty connectivity...");
    match ghostty::health_check().await {
        Ok(()) => tracing::info!("Ghostty IPC socket is reachable"),
        Err(e) => {
            eprintln!("error: Cannot connect to Ghostty: {e}");
            eprintln!();
            eprintln!("Make sure Ghostty is running. The bridge connects to:");
            eprintln!("  $XDG_RUNTIME_DIR/ghostty/sock  (Linux)");
            eprintln!("  $TMPDIR/ghostty-<uid>.sock      (macOS)");
            eprintln!("Override with: GHOSTTY_SOCKET=/path/to/sock");
            process::exit(1);
        }
    }

    // Report which input backend will be used
    let backend = ghostty::InputBackend::detect();
    if backend == ghostty::InputBackend::None {
        tracing::warn!(
            "No input backend detected. \
             Install xdotool (Linux/X11) or enable Accessibility (macOS) for \
             Input.sendText / Input.sendKeys support."
        );
    } else {
        tracing::info!("Input backend: {:?}", backend);
    }

    let port = match cli.port {
        Some(p) => p,
        None => find_available_port(&cli.host, PORT_RANGE_START, PORT_RANGE_END)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No available port in range {PORT_RANGE_START}-{PORT_RANGE_END}"
                )
            })?,
    };

    let addr: SocketAddr = format!("{}:{}", cli.host, port).parse()?;

    let module = build_rpc_module(None, None)?;
    let server = Server::builder().build(addr).await?;
    let handle = server.start(module);

    tracing::info!("wrightty-bridge-ghostty listening on ws://{addr}");
    println!("wrightty-bridge-ghostty listening on ws://{addr}");

    // --- Watchdog: periodically check Ghostty is still running ---
    if cli.watchdog_interval > 0 {
        let interval = Duration::from_secs(cli.watchdog_interval);
        let server_handle = handle.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            loop {
                tokio::time::sleep(interval).await;
                match ghostty::health_check().await {
                    Ok(()) => {
                        if consecutive_failures > 0 {
                            tracing::info!("Ghostty reconnected");
                            consecutive_failures = 0;
                        }
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        tracing::warn!(
                            "Ghostty health check failed ({consecutive_failures}): {e}"
                        );
                        if consecutive_failures >= 3 {
                            tracing::error!(
                                "Ghostty unreachable after {consecutive_failures} checks, shutting down"
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
