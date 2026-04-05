use std::collections::HashMap;

use clap::{Args, Subcommand};
use wrightty_client::WrighttyClient;
use wrightty_protocol::types::*;

use crate::server::{PORT_RANGE_START, PORT_RANGE_END};

/// Convert `Box<dyn Error>` from WrighttyClient into anyhow::Error.
fn e<T>(r: Result<T, Box<dyn std::error::Error>>) -> anyhow::Result<T> {
    r.map_err(|e| anyhow::anyhow!("{e}"))
}

// --- Shared connection logic ---

#[derive(Args, Clone)]
pub struct ConnectOpts {
    /// Server URL (default: auto-discover)
    #[arg(long, global = true)]
    url: Option<String>,

    /// Session ID (default: first available)
    #[arg(long, global = true)]
    session: Option<String>,
}

async fn connect(opts: &ConnectOpts) -> anyhow::Result<(WrighttyClient, String)> {
    let url = match &opts.url {
        Some(u) => u.clone(),
        None => discover_first().await?,
    };

    let client = e(WrighttyClient::connect(&url).await)
        .map_err(|err| anyhow::anyhow!("Failed to connect to {url}: {err}"))?;

    let session_id = match &opts.session {
        Some(s) => s.clone(),
        None => {
            let sessions = e(client.session_list().await)?;
            sessions
                .first()
                .map(|s| s.session_id.clone())
                .unwrap_or_else(|| "0".to_string())
        }
    };

    Ok((client, session_id))
}

async fn discover_first() -> anyhow::Result<String> {
    use jsonrpsee::core::client::ClientT;
    use jsonrpsee::core::params::ObjectParams;
    use jsonrpsee::ws_client::WsClientBuilder;

    for port in PORT_RANGE_START..=PORT_RANGE_END {
        let url = format!("ws://127.0.0.1:{port}");
        let Ok(client) = WsClientBuilder::default()
            .connection_timeout(std::time::Duration::from_millis(100))
            .build(&url)
            .await
        else {
            continue;
        };
        let Ok(_): Result<serde_json::Value, _> =
            client.request("Wrightty.getInfo", ObjectParams::new()).await
        else {
            continue;
        };
        return Ok(url);
    }

    anyhow::bail!(
        "No wrightty server found on ports {PORT_RANGE_START}-{PORT_RANGE_END}.\n\
         Start one with:\n  \
         wrightty term --headless\n  \
         wrightty term --bridge-tmux\n  \
         wrightty term --bridge-wezterm"
    )
}

// --- Commands ---

#[derive(Args)]
pub struct RunArgs {
    /// The command to run
    command: String,

    /// Timeout in seconds
    #[arg(long, default_value_t = 30)]
    timeout: u64,

    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn run_cmd(args: RunArgs) -> anyhow::Result<()> {
    let (client, session_id) = connect(&args.connect).await?;

    // Send command
    e(client
        .send_text(&session_id, &format!("{}\n", args.command))
        .await)?;

    // Wait for prompt
    let _result = e(client
        .wait_for_text(&session_id, r"[$#>%]\s*$", true, args.timeout * 1000)
        .await)?;

    // Read screen and extract output
    let text = e(client.get_text(&session_id).await)?;
    let lines: Vec<&str> = text.trim().split('\n').collect();

    let mut output_lines = Vec::new();
    let mut found_cmd = false;
    for line in &lines {
        if !found_cmd {
            if line.contains(&args.command) {
                found_cmd = true;
            }
            continue;
        }
        if is_prompt_line(line) {
            break;
        }
        output_lines.push(*line);
    }

    if output_lines.is_empty() && !found_cmd {
        println!("{text}");
    } else {
        println!("{}", output_lines.join("\n"));
    }

    Ok(())
}

fn is_prompt_line(text: &str) -> bool {
    let t = text.trim_end();
    t.ends_with('$') || t.ends_with('#') || t.ends_with('>') || t.ends_with('%')
}

#[derive(Args)]
pub struct ReadArgs {
    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn read_cmd(args: ReadArgs) -> anyhow::Result<()> {
    let (client, session_id) = connect(&args.connect).await?;
    let text = e(client.get_text(&session_id).await)?;
    println!("{text}");
    Ok(())
}

#[derive(Args)]
pub struct SendTextArgs {
    /// Text to send (use \\n for newline)
    text: String,

    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn send_text_cmd(args: SendTextArgs) -> anyhow::Result<()> {
    let (client, session_id) = connect(&args.connect).await?;
    let text = args.text.replace("\\n", "\n");
    e(client.send_text(&session_id, &text).await)?;
    Ok(())
}

#[derive(Args)]
pub struct SendKeysArgs {
    /// Keys to send (e.g. Ctrl+c Escape Enter)
    keys: Vec<String>,

    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn send_keys_cmd(args: SendKeysArgs) -> anyhow::Result<()> {
    let (client, session_id) = connect(&args.connect).await?;
    let keys: Vec<KeyInput> = args
        .keys
        .iter()
        .map(|k| KeyInput::Shorthand(k.clone()))
        .collect();
    e(client.send_keys(&session_id, keys).await)?;
    Ok(())
}

#[derive(Args)]
pub struct ScreenshotArgs {
    /// Format: text, svg, png, json
    #[arg(long, default_value = "text")]
    format: String,

    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<String>,

    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn screenshot_cmd(args: ScreenshotArgs) -> anyhow::Result<()> {
    let (client, session_id) = connect(&args.connect).await?;

    let format = match args.format.as_str() {
        "text" => ScreenshotFormat::Text,
        "svg" => ScreenshotFormat::Svg,
        "png" => ScreenshotFormat::Png,
        "json" => ScreenshotFormat::Json,
        other => anyhow::bail!("Unknown format: {other}"),
    };

    let result = e(client.screenshot(&session_id, format).await)?;

    match args.output {
        Some(path) => {
            std::fs::write(&path, &result.data)?;
            println!("Screenshot saved to {path}");
        }
        None => print!("{}", result.data),
    }
    Ok(())
}

#[derive(Args)]
pub struct WaitForArgs {
    /// Pattern to wait for
    pattern: String,

    /// Timeout in seconds
    #[arg(long, default_value_t = 30)]
    timeout: u64,

    /// Treat pattern as regex
    #[arg(long)]
    regex: bool,

    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn wait_for_cmd(args: WaitForArgs) -> anyhow::Result<()> {
    let (client, session_id) = connect(&args.connect).await?;

    let result = e(client
        .wait_for_text(
            &session_id,
            &args.pattern,
            args.regex,
            args.timeout * 1000,
        )
        .await)?;

    if result.found {
        let text = e(client.get_text(&session_id).await)?;
        println!("{text}");
    } else {
        eprintln!(
            "Timeout: '{}' not found within {}s",
            args.pattern, args.timeout
        );
        std::process::exit(1);
    }
    Ok(())
}

#[derive(Args)]
pub struct InfoArgs {
    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn info_cmd(args: InfoArgs) -> anyhow::Result<()> {
    let (client, _) = connect(&args.connect).await?;
    let info = e(client.get_info().await)?;
    println!("{}", serde_json::to_string_pretty(&info)?);
    Ok(())
}

#[derive(Args)]
pub struct SizeArgs {
    #[command(flatten)]
    connect: ConnectOpts,
}

pub async fn size_cmd(args: SizeArgs) -> anyhow::Result<()> {
    let (client, session_id) = connect(&args.connect).await?;
    let (cols, rows) = e(client.get_size(&session_id).await)?;
    println!("{cols}x{rows}");
    Ok(())
}

#[derive(Args)]
pub struct SessionArgs {
    #[command(subcommand)]
    command: SessionCommand,

    #[command(flatten)]
    connect: ConnectOpts,
}

#[derive(Subcommand)]
pub enum SessionCommand {
    /// List all sessions
    List,
    /// Create a new session (headless mode only)
    Create {
        /// Shell to use
        #[arg(long)]
        shell: Option<String>,
        /// Columns
        #[arg(long, default_value_t = 120)]
        cols: u16,
        /// Rows
        #[arg(long, default_value_t = 40)]
        rows: u16,
    },
    /// Destroy a session
    Destroy {
        /// Session ID to destroy
        id: String,
    },
}

pub async fn session_cmd(args: SessionArgs) -> anyhow::Result<()> {
    let (client, _) = connect(&args.connect).await?;

    match args.command {
        SessionCommand::List => {
            let sessions = e(client.session_list().await)?;
            if sessions.is_empty() {
                println!("No sessions.");
            } else {
                for s in sessions {
                    println!(
                        "  {} {}x{} {}",
                        s.session_id,
                        s.cols,
                        s.rows,
                        s.title
                    );
                }
            }
        }
        SessionCommand::Create { shell, cols, rows } => {
            let params = wrightty_protocol::methods::SessionCreateParams {
                shell,
                args: vec![],
                cols,
                rows,
                env: HashMap::new(),
                cwd: None,
            };
            let id = e(client.session_create(params).await)?;
            println!("{id}");
        }
        SessionCommand::Destroy { id } => {
            e(client.session_destroy(&id).await)?;
            println!("Destroyed session {id}");
        }
    }
    Ok(())
}
