use clap::Args;

use crate::server::{PORT_RANGE_START, PORT_RANGE_END};

#[derive(Args)]
pub struct DiscoverArgs {
    /// Host to scan
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

pub async fn run(args: DiscoverArgs) -> anyhow::Result<()> {
    let mut found = Vec::new();

    for port in PORT_RANGE_START..=PORT_RANGE_END {
        let url = format!("ws://{}:{}", args.host, port);
        match try_connect(&url).await {
            Some(info) => found.push(info),
            None => continue,
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&found)?);
        return Ok(());
    }

    if found.is_empty() {
        println!("No wrightty servers found on ports {PORT_RANGE_START}-{PORT_RANGE_END}.");
        println!();
        println!("Start one with:");
        println!("  wrightty term --headless");
        println!("  wrightty term --bridge-tmux");
        println!("  wrightty term --bridge-wezterm");
        return Ok(());
    }

    for s in &found {
        println!(
            "  {}  {} v{}",
            s["url"].as_str().unwrap_or(""),
            s["implementation"].as_str().unwrap_or(""),
            s["version"].as_str().unwrap_or(""),
        );
    }

    Ok(())
}

async fn try_connect(url: &str) -> Option<serde_json::Value> {
    use jsonrpsee::core::client::ClientT;
    use jsonrpsee::core::params::ObjectParams;
    use jsonrpsee::ws_client::WsClientBuilder;

    let client = WsClientBuilder::default()
        .connection_timeout(std::time::Duration::from_millis(200))
        .build(url)
        .await
        .ok()?;

    let info: serde_json::Value = client
        .request("Wrightty.getInfo", ObjectParams::new())
        .await
        .ok()?;

    let mut result = serde_json::json!({ "url": url });
    if let Some(obj) = info.as_object() {
        for (k, v) in obj {
            result[k] = v.clone();
        }
    }

    Some(result)
}
