mod api;
mod config;
mod display;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ghw", about = "GitHub notifications without the noise")]
struct Cli {
    #[arg(short = 'v', long = "version", help = "Print version and exit", global = true)]
    version: bool,

    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// PRs and issues where you were @mentioned
    Mentions {
        #[arg(short, long, help = "Include already-read notifications")]
        all: bool,
        #[arg(short, long, help = "Mark mention notifications as read")]
        clear: bool,
    },
    /// Reviews and comments on your own PRs
    #[command(name = "my-prs", alias = "prs")]
    MyPrs {
        #[arg(short, long, help = "Include already-read notifications")]
        all: bool,
        #[arg(short, long, help = "Mark PR notifications as read")]
        clear: bool,
    },
    /// Conversations you commented on
    Threads {
        #[arg(short, long, help = "Include already-read notifications")]
        all: bool,
        #[arg(short, long, help = "Mark thread notifications as read")]
        clear: bool,
    },
    /// Recent activity in your watched repos
    Feed {
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Add a repo to your watch list (owner/repo)
    Watch { repo: String },
    /// Remove a repo from your watch list
    Unwatch { repo: String },
    /// List your watched repos
    Watched,
    /// All of the above at once
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("ghw {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    display::migrate_legacy_state(); // fixing a bug, will delete this later
    let client = api::Client::new()?;

    match cli.command.unwrap_or(Cmd::Status) {
        Cmd::Mentions { all, clear } => display::mentions(&client, all, clear),
        Cmd::MyPrs { all, clear } => display::my_prs(&client, all, clear),
        Cmd::Threads { all, clear } => display::threads(&client, all, clear),
        Cmd::Feed { limit }      => display::feed(&client, limit),
        Cmd::Watch { repo }      => config::watch(&client, &repo),
        Cmd::Unwatch { repo }    => config::unwatch(&repo),
        Cmd::Watched             => config::list_watched(),
        Cmd::Status              => {
            display::mentions(&client, false, false)?;
            display::my_prs(&client, false, false)?;
            display::threads(&client, false, false)?;
            display::feed(&client, 8)
        }
    }
}
