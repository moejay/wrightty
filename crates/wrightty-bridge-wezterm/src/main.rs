use std::net::SocketAddr;

use clap::Parser;
use jsonrpsee::server::Server;
use tracing_subscriber::EnvFilter;

use wrightty_bridge_wezterm::rpc::build_rpc_module;

#[derive(Parser)]
#[command(
    name = "wrightty-bridge-wezterm",
    about = "Bridge that translates wrightty protocol calls into wezterm cli commands"
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 9421)]
    port: u16,
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
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    let module = build_rpc_module()?;

    let server = Server::builder().build(addr).await?;

    let handle = server.start(module);

    tracing::info!("wrightty-bridge-wezterm listening on ws://{addr}");
    println!("wrightty-bridge-wezterm listening on ws://{addr}");

    handle.stopped().await;

    Ok(())
}
