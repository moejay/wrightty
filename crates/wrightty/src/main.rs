use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod discover;
mod server;
mod term;
mod version;

#[cfg(feature = "client")]
mod client_cmds;

#[derive(Parser)]
#[command(
    name = "wrightty",
    about = "Wrightty — Playwright for terminals",
    version,
    after_help = "Examples:\n  wrightty term --headless          Start headless terminal server\n  wrightty term --bridge-tmux       Bridge to a running tmux session\n  wrightty run \"ls -la\"              Run a command and print output\n  wrightty discover                  Find running wrightty servers"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a wrightty terminal server or bridge
    Term(term::TermArgs),

    /// Discover running wrightty servers
    Discover(discover::DiscoverArgs),

    /// Check for updates and upgrade wrightty
    Upgrade,

    /// Run a command and print its output
    #[cfg(feature = "client")]
    Run(client_cmds::RunArgs),

    /// Read the current terminal screen
    #[cfg(feature = "client")]
    Read(client_cmds::ReadArgs),

    /// Send raw text to the terminal
    #[cfg(feature = "client")]
    SendText(client_cmds::SendTextArgs),

    /// Send keystrokes to the terminal
    #[cfg(feature = "client")]
    SendKeys(client_cmds::SendKeysArgs),

    /// Take a terminal screenshot
    #[cfg(feature = "client")]
    Screenshot(client_cmds::ScreenshotArgs),

    /// Wait until text appears on screen
    #[cfg(feature = "client")]
    WaitFor(client_cmds::WaitForArgs),

    /// Show server info and capabilities
    #[cfg(feature = "client")]
    Info(client_cmds::InfoArgs),

    /// Get terminal dimensions
    #[cfg(feature = "client")]
    Size(client_cmds::SizeArgs),

    /// Manage sessions
    #[cfg(feature = "client")]
    Session(client_cmds::SessionArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("wrightty=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    // Background version check (non-blocking, cached 24h)
    if !matches!(cli.command, Commands::Upgrade | Commands::Term(_)) {
        version::check_in_background();
    }

    match cli.command {
        Commands::Term(args) => term::run(args).await,
        Commands::Discover(args) => discover::run(args).await,
        Commands::Upgrade => version::upgrade().await,
        #[cfg(feature = "client")]
        Commands::Run(args) => client_cmds::run_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::Read(args) => client_cmds::read_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::SendText(args) => client_cmds::send_text_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::SendKeys(args) => client_cmds::send_keys_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::Screenshot(args) => client_cmds::screenshot_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::WaitFor(args) => client_cmds::wait_for_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::Info(args) => client_cmds::info_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::Size(args) => client_cmds::size_cmd(args).await,
        #[cfg(feature = "client")]
        Commands::Session(args) => client_cmds::session_cmd(args).await,
    }
}
