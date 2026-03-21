use std::net::SocketAddr;

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_server::rpc::build_rpc_module;
use wrightty_server::state::AppState;

#[derive(Parser)]
#[command(name = "wrightty-server", about = "Wrightty terminal automation daemon")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 9420)]
    port: u16,

    #[arg(long, default_value_t = 64)]
    max_sessions: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("wrightty=info".parse()?))
        .init();

    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    let state = AppState::new(cli.max_sessions);
    let module = build_rpc_module(state)?;

    let server = Server::builder().build(addr).await?;

    let handle = server.start(module);

    tracing::info!("wrightty-server listening on ws://{addr}");
    println!("wrightty-server listening on ws://{addr}");

    handle.stopped().await;

    Ok(())
}
