use std::process;

use clap::Args;

use crate::server::{self, PORT_RANGE_START, PORT_RANGE_END};

#[derive(Args)]
pub struct TermArgs {
    #[command(flatten)]
    mode: TermMode,

    /// Host to bind on
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on (default: auto-select)
    #[arg(long)]
    port: Option<u16>,

    /// Max sessions for headless mode
    #[arg(long, default_value_t = 64)]
    max_sessions: usize,

    /// Watchdog interval in seconds for bridges (0 to disable)
    #[arg(long, default_value_t = 5)]
    watchdog_interval: u64,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct TermMode {
    /// Start the headless terminal server (virtual PTY, no GUI)
    #[cfg(feature = "headless")]
    #[arg(long)]
    headless: bool,

    /// Launch the Alacritty fork with built-in wrightty support
    #[arg(long)]
    alacritty: bool,

    /// Bridge to a running WezTerm instance
    #[cfg(feature = "bridge-wezterm")]
    #[arg(long)]
    bridge_wezterm: bool,

    /// Bridge to a running tmux server
    #[cfg(feature = "bridge-tmux")]
    #[arg(long)]
    bridge_tmux: bool,

    /// Bridge to a running Kitty instance
    #[cfg(feature = "bridge-kitty")]
    #[arg(long)]
    bridge_kitty: bool,

    /// Bridge to a running Zellij session
    #[cfg(feature = "bridge-zellij")]
    #[arg(long)]
    bridge_zellij: bool,

    /// Bridge to a running Ghostty instance
    #[cfg(feature = "bridge-ghostty")]
    #[arg(long)]
    bridge_ghostty: bool,
}

pub async fn run(args: TermArgs) -> anyhow::Result<()> {
    #[cfg(feature = "headless")]
    if args.mode.headless {
        return run_headless(args).await;
    }

    if args.mode.alacritty {
        return run_alacritty(args).await;
    }

    #[cfg(feature = "bridge-wezterm")]
    if args.mode.bridge_wezterm {
        return run_bridge_wezterm(args).await;
    }

    #[cfg(feature = "bridge-tmux")]
    if args.mode.bridge_tmux {
        return run_bridge_tmux(args).await;
    }

    #[cfg(feature = "bridge-kitty")]
    if args.mode.bridge_kitty {
        return run_bridge_kitty(args).await;
    }

    #[cfg(feature = "bridge-zellij")]
    if args.mode.bridge_zellij {
        return run_bridge_zellij(args).await;
    }

    #[cfg(feature = "bridge-ghostty")]
    if args.mode.bridge_ghostty {
        return run_bridge_ghostty(args).await;
    }

    anyhow::bail!("No terminal mode selected. Use --headless, --bridge-tmux, etc.")
}

fn resolve_port(args: &TermArgs) -> anyhow::Result<u16> {
    match args.port {
        Some(p) => Ok(p),
        None => server::find_available_port(&args.host, PORT_RANGE_START, PORT_RANGE_END)
            .ok_or_else(|| {
                anyhow::anyhow!("No available port in range {PORT_RANGE_START}-{PORT_RANGE_END}")
            }),
    }
}

// --- Headless mode ---

#[cfg(feature = "headless")]
async fn run_headless(args: TermArgs) -> anyhow::Result<()> {
    let port = resolve_port(&args)?;
    let state = wrightty_server::state::AppState::new(args.max_sessions);
    let module = wrightty_server::rpc::build_rpc_module(state)?;
    server::start_server(&args.host, port, "wrightty (headless)", module).await
}

// --- Alacritty mode ---

async fn run_alacritty(args: TermArgs) -> anyhow::Result<()> {
    let port = args.port.unwrap_or(PORT_RANGE_START);

    // Find the alacritty binary — check for wrightty-patched version
    let alacritty = which_alacritty()?;

    tracing::info!("Launching {alacritty} --wrightty {port}");
    println!("Launching {alacritty} --wrightty {port}");

    let status = tokio::process::Command::new(&alacritty)
        .arg("--wrightty")
        .arg(port.to_string())
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("Alacritty exited with status {status}");
    }

    Ok(())
}

fn which_alacritty() -> anyhow::Result<String> {
    // Check PATH for alacritty
    let output = std::process::Command::new("which")
        .arg("alacritty")
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            // Verify it supports --wrightty
            let check = std::process::Command::new(&path)
                .arg("--help")
                .output();
            if let Ok(help) = check {
                let help_text = String::from_utf8_lossy(&help.stdout);
                if help_text.contains("wrightty") {
                    return Ok(path);
                }
            }
            anyhow::bail!(
                "Found alacritty at {path} but it doesn't support --wrightty.\n\
                 Install the wrightty-patched fork:\n  \
                 git clone -b wrightty-support https://github.com/moejay/alacritty.git\n  \
                 cd alacritty && cargo install --path alacritty --features wrightty"
            );
        }
        _ => anyhow::bail!(
            "alacritty not found in PATH.\n\
             Install the wrightty-patched fork:\n  \
             git clone -b wrightty-support https://github.com/moejay/alacritty.git\n  \
             cd alacritty && cargo install --path alacritty --features wrightty"
        ),
    }
}

// --- Bridge modes ---

#[cfg(feature = "bridge-wezterm")]
async fn run_bridge_wezterm(args: TermArgs) -> anyhow::Result<()> {
    tracing::info!("Checking WezTerm connectivity...");
    match wrightty_bridge_wezterm::wezterm::health_check().await {
        Ok(()) => tracing::info!("WezTerm is reachable"),
        Err(e) => {
            eprintln!("error: Cannot connect to WezTerm: {e}");
            eprintln!();
            eprintln!("Make sure WezTerm is running. If using flatpak, set:");
            eprintln!(
                "  WEZTERM_CMD=\"flatpak run --command=wezterm org.wezfurlong.wezterm\""
            );
            process::exit(1);
        }
    }

    let port = resolve_port(&args)?;
    let module = wrightty_bridge_wezterm::rpc::build_rpc_module()?;
    server::start_server_with_watchdog(
        &args.host,
        port,
        "wrightty (wezterm bridge)",
        module,
        args.watchdog_interval,
        || async { wrightty_bridge_wezterm::wezterm::health_check().await.map_err(|e| e.into()) },
    )
    .await
}

#[cfg(feature = "bridge-tmux")]
async fn run_bridge_tmux(args: TermArgs) -> anyhow::Result<()> {
    tracing::info!("Checking tmux connectivity...");
    match wrightty_bridge_tmux::tmux::health_check().await {
        Ok(()) => tracing::info!("tmux is reachable"),
        Err(e) => {
            eprintln!("error: Cannot connect to tmux: {e}");
            eprintln!();
            eprintln!("Make sure a tmux server is running. Start one with:");
            eprintln!("  tmux new-session -d -s main");
            process::exit(1);
        }
    }

    let port = resolve_port(&args)?;
    let module = wrightty_bridge_tmux::rpc::build_rpc_module()?;
    server::start_server_with_watchdog(
        &args.host,
        port,
        "wrightty (tmux bridge)",
        module,
        args.watchdog_interval,
        || async { wrightty_bridge_tmux::tmux::health_check().await.map_err(|e| e.into()) },
    )
    .await
}

#[cfg(feature = "bridge-kitty")]
async fn run_bridge_kitty(args: TermArgs) -> anyhow::Result<()> {
    tracing::info!("Checking kitty connectivity...");
    match wrightty_bridge_kitty::kitty::health_check().await {
        Ok(()) => tracing::info!("kitty is reachable"),
        Err(e) => {
            eprintln!("error: Cannot connect to kitty: {e}");
            eprintln!();
            eprintln!("Make sure kitty is running with remote control enabled.");
            eprintln!("Add to kitty.conf:");
            eprintln!("  allow_remote_control yes");
            eprintln!("Or launch kitty with:");
            eprintln!("  kitty --listen-on unix:/tmp/kitty.sock");
            process::exit(1);
        }
    }

    let port = resolve_port(&args)?;
    let module = wrightty_bridge_kitty::rpc::build_rpc_module()?;
    server::start_server_with_watchdog(
        &args.host,
        port,
        "wrightty (kitty bridge)",
        module,
        args.watchdog_interval,
        || async { wrightty_bridge_kitty::kitty::health_check().await.map_err(|e| e.into()) },
    )
    .await
}

#[cfg(feature = "bridge-zellij")]
async fn run_bridge_zellij(args: TermArgs) -> anyhow::Result<()> {
    tracing::info!("Checking zellij connectivity...");
    match wrightty_bridge_zellij::zellij::health_check().await {
        Ok(()) => tracing::info!("zellij is reachable"),
        Err(e) => {
            eprintln!("error: Cannot connect to zellij: {e}");
            eprintln!();
            eprintln!("This bridge must run from within a zellij session.");
            eprintln!("Start zellij first:");
            eprintln!("  zellij");
            process::exit(1);
        }
    }

    let port = resolve_port(&args)?;
    let module = wrightty_bridge_zellij::rpc::build_rpc_module()?;
    server::start_server_with_watchdog(
        &args.host,
        port,
        "wrightty (zellij bridge)",
        module,
        args.watchdog_interval,
        || async { wrightty_bridge_zellij::zellij::health_check().await.map_err(|e| e.into()) },
    )
    .await
}

#[cfg(feature = "bridge-ghostty")]
async fn run_bridge_ghostty(args: TermArgs) -> anyhow::Result<()> {
    tracing::info!("Checking Ghostty connectivity...");
    match wrightty_bridge_ghostty::ghostty::health_check().await {
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

    let backend = wrightty_bridge_ghostty::ghostty::InputBackend::detect();
    if backend == wrightty_bridge_ghostty::ghostty::InputBackend::None {
        tracing::warn!(
            "No input backend detected. \
             Install xdotool (Linux/X11) or enable Accessibility (macOS) for \
             Input.sendText / Input.sendKeys support."
        );
    } else {
        tracing::info!("Input backend: {:?}", backend);
    }

    let port = resolve_port(&args)?;
    let module = wrightty_bridge_ghostty::rpc::build_rpc_module()?;
    server::start_server_with_watchdog(
        &args.host,
        port,
        "wrightty (ghostty bridge)",
        module,
        args.watchdog_interval,
        || async { wrightty_bridge_ghostty::ghostty::health_check().await.map_err(|e| e.into()) },
    )
    .await
}
