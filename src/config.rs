use anyhow::{anyhow, Result};
use std::path::PathBuf;

use crate::api::Client;

fn repos_file() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ghw-repos")
}

pub fn load() -> Vec<String> {
    std::fs::read_to_string(repos_file())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect()
}

fn save(repos: &[String]) -> Result<()> {
    std::fs::write(repos_file(), repos.join("\n"))?;
    Ok(())
}

pub fn watch(client: &Client, repo: &str) -> Result<()> {
    let mut repos = load();
    if repos.iter().any(|r| r == repo) {
        println!("Already watching {repo}.");
        return Ok(());
    }
    if !client.repo_exists(repo) {
        return Err(anyhow!("Repo '{repo}' not found or not accessible."));
    }
    repos.push(repo.to_string());
    save(&repos)?;
    println!("✓ Now watching {repo}");
    Ok(())
}

pub fn unwatch(repo: &str) -> Result<()> {
    let mut repos = load();
    let before = repos.len();
    repos.retain(|r| r != repo);
    if repos.len() == before {
        println!("{repo} is not in your watch list.");
    } else {
        save(&repos)?;
        println!("✓ Stopped watching {repo}");
    }
    Ok(())
}

pub fn list_watched() -> Result<()> {
    let repos = load();
    if repos.is_empty() {
        println!("No repos watched yet.  Use: ghw watch owner/repo");
    } else {
        println!("\nWatched repos:");
        for r in &repos {
            println!("  · {r}");
        }
    }
    Ok(())
}
