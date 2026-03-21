use std::net::{SocketAddr, TcpListener};

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_bridge_wezterm::rpc::build_rpc_module;

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

    handle.stopped().await;

    Ok(())
}
