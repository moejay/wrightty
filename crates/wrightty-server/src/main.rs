use std::net::{SocketAddr, TcpListener};

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_server::rpc::build_rpc_module;
use wrightty_server::state::AppState;

const PORT_RANGE_START: u16 = 9420;
const PORT_RANGE_END: u16 = 9440;

#[derive(Parser)]
#[command(name = "wrightty-server", about = "Wrightty terminal automation daemon")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on. If not specified, auto-selects the next available port starting at 9420.
    #[arg(long)]
    port: Option<u16>,

    #[arg(long, default_value_t = 64)]
    max_sessions: usize,
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
        .with_env_filter(EnvFilter::from_default_env().add_directive("wrightty=info".parse()?))
        .init();

    let cli = Cli::parse();

    let port = match cli.port {
        Some(p) => p,
        None => find_available_port(&cli.host, PORT_RANGE_START, PORT_RANGE_END)
            .ok_or_else(|| anyhow::anyhow!("No available port in range {PORT_RANGE_START}-{PORT_RANGE_END}"))?,
    };

    let addr: SocketAddr = format!("{}:{}", cli.host, port).parse()?;

    let state = AppState::new(cli.max_sessions, None, None);
    let module = build_rpc_module(state)?;

    let server = Server::builder().build(addr).await?;

    let handle = server.start(module);

    tracing::info!("wrightty-server listening on ws://{addr}");
    println!("wrightty-server listening on ws://{addr}");

    handle.stopped().await;

    Ok(())
}
