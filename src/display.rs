use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::api::Client;
use crate::config;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn time_ago(dt: &DateTime<Utc>) -> String {
    let s = (Utc::now() - *dt).num_seconds().max(0) as u64;
    match s {
        0..=59       => format!("{s}s ago"),
        60..=3599    => format!("{}m ago", s / 60),
        3600..=86399 => format!("{}h ago", s / 3600),
        _            => format!("{}d ago", s / 86400),
    }
}

fn header(text: &str) {
    println!("\n\x1b[1m{text}\x1b[0m");
}

fn dim(s: &str) -> String { format!("\x1b[2m{s}\x1b[0m") }
fn cyan(s: &str) -> String { format!("\x1b[36m{s}\x1b[0m") }
fn yellow(s: &str) -> String { format!("\x1b[33m{s}\x1b[0m") }
fn magenta(s: &str) -> String { format!("\x1b[35m{s}\x1b[0m") }
fn green(s: &str) -> String { format!("\x1b[32m{s}\x1b[0m") }

// ── Commands ──────────────────────────────────────────────────────────────────

pub fn mentions(client: &Client, all: bool) -> Result<()> {
    header("── Mentions ──────────────────────────────────");
    let notes = client.notifications(all)?;
    let hits: Vec<_> = notes.iter().filter(|n| n.reason == "mention").collect();

    if hits.is_empty() {
        println!("  {}", green("✓ No unread mentions."));
        return Ok(());
    }

    println!("  {} mention(s)\n", hits.len());
    for n in hits {
        let icon = if n.subject.kind == "PullRequest" { "⎇" } else { "◉" };
        println!("  {icon}  {}  {}", cyan(&n.repository.full_name), n.subject.title);
        println!("      {}", dim(&time_ago(&n.updated_at)));
        println!();
    }
    Ok(())
}

pub fn my_prs(client: &Client, all: bool) -> Result<()> {
    header("── My PRs ────────────────────────────────────");
    let notes = client.notifications(all)?;
    let relevant = ["review_requested", "comment", "author"];
    let hits: Vec<_> = notes.iter()
        .filter(|n| n.subject.kind == "PullRequest" && relevant.contains(&n.reason.as_str()))
        .collect();

    if hits.is_empty() {
        println!("  {}", green("✓ No new activity on your PRs."));
        return Ok(());
    }

    println!("  {} PR update(s)\n", hits.len());
    for n in hits {
        let reason = n.reason.replace('_', " ");
        println!("  ⎇  {}  {}", cyan(&n.repository.full_name), n.subject.title);
        println!("      {}  ·  {}", yellow(&reason), dim(&time_ago(&n.updated_at)));
        println!();
    }
    Ok(())
}

pub fn feed(client: &Client, limit: usize) -> Result<()> {
    header("── Feed ──────────────────────────────────────");
    let repos = config::load();

    if repos.is_empty() {
        println!("  No repos watched.  Add one with: ghw watch owner/repo");
        return Ok(());
    }

    for repo in &repos {
        let events = match client.events(repo, limit) {
            Ok(e) => e,
            Err(e) => { eprintln!("  Skipping {repo}: {e}"); continue; }
        };

        println!("\n  {}", cyan(&format!("── {repo}")));

        for ev in events.iter().take(limit) {
            let kind = ev.kind.trim_end_matches("Event");
            let p = &ev.payload;

            let detail = match kind {
                "Push" => {
                    let n = p["commits"].as_array().map(|a| a.len()).unwrap_or(0);
                    let branch = p["ref"].as_str().unwrap_or("").split('/').last().unwrap_or("");
                    format!("{n} commit(s) → {branch}")
                }
                "PullRequest" => format!(
                    "{} #{} {}",
                    p["action"].as_str().unwrap_or(""),
                    p["number"].as_u64().unwrap_or(0),
                    p["pull_request"]["title"].as_str().unwrap_or(""),
                ),
                "Issues" => format!(
                    "{} #{} {}",
                    p["action"].as_str().unwrap_or(""),
                    p["issue"]["number"].as_u64().unwrap_or(0),
                    p["issue"]["title"].as_str().unwrap_or(""),
                ),
                "IssueComment" => format!(
                    "comment on #{}",
                    p["issue"]["number"].as_u64().unwrap_or(0)
                ),
                "Create" => format!(
                    "created {} {}",
                    p["ref_type"].as_str().unwrap_or(""),
                    p["ref"].as_str().unwrap_or(""),
                ),
                "Delete" => format!(
                    "deleted {} {}",
                    p["ref_type"].as_str().unwrap_or(""),
                    p["ref"].as_str().unwrap_or(""),
                ),
                _ => String::new(),
            };

            println!(
                "  {:<20}  \x1b[1m{}\x1b[0m  {detail}",
                magenta(kind),
                ev.actor.login,
            );
            println!("  {:<20}  {}", "", dim(&time_ago(&ev.created_at)));
            println!();
        }
    }
    Ok(())
}
