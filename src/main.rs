mod api;
mod config;
mod display;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ghw", about = "GitHub notifications without the noise", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// PRs and issues where you were @mentioned
    Mentions {
        #[arg(short, long, help = "Include already-read notifications")]
        all: bool,
    },
    /// Reviews and comments on your own PRs
    #[command(name = "my-prs")]
    MyPrs {
        #[arg(short, long, help = "Include already-read notifications")]
        all: bool,
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
    let client = api::Client::new()?;

    match cli.command.unwrap_or(Cmd::Status) {
        Cmd::Mentions { all }    => display::mentions(&client, all),
        Cmd::MyPrs { all }       => display::my_prs(&client, all),
        Cmd::Feed { limit }      => display::feed(&client, limit),
        Cmd::Watch { repo }      => config::watch(&client, &repo),
        Cmd::Unwatch { repo }    => config::unwatch(&repo),
        Cmd::Watched             => config::list_watched(),
        Cmd::Status              => {
            display::mentions(&client, false)?;
            display::my_prs(&client, false)?;
            display::feed(&client, 8)
        }
    }
}
