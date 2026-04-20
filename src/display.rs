use std::collections::HashMap;
use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::api::{Client, Notification};
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
fn red(s: &str) -> String { format!("\x1b[31m{s}\x1b[0m") }

fn truncate_body(body: &str, max: usize) -> String {
    let single_line: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.len() <= max {
        single_line
    } else {
        format!("{}...", &single_line[..max])
    }
}

fn print_comment_info(client: &Client, n: &Notification) {
    if let Some(url) = &n.subject.latest_comment_url {
        if let Ok(comment) = client.comment(url) {
            println!("      {}  {}", yellow(&comment.user.login), dim(&truncate_body(&comment.body, 120)));
        }
    }
}

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
        print_comment_info(client, n);
        println!();
    }
    Ok(())
}

pub fn my_prs(client: &Client, all: bool) -> Result<()> {
    header("── My PRs ────────────────────────────────────");
    let notes = client.notifications(all)?;
    let relevant = ["author"];
    let hits: Vec<_> = notes.iter()
        .filter(|n| n.subject.kind == "PullRequest" && relevant.contains(&n.reason.as_str()))
        .collect();

    if hits.is_empty() {
        println!("  {}", green("✓ No new activity on your PRs."));
        return Ok(());
    }

    println!("  {} PR update(s)\n", hits.len());
    for n in hits {
        println!("  ⎇  {}  {}", cyan(&n.repository.full_name), n.subject.title);

        // Fetch and display reviews
        if let Some(url) = &n.subject.url {
            if let Ok(reviews) = client.reviews(url) {
                // Deduplicate: keep only the latest review per reviewer
                let mut latest_by_user: HashMap<String, &crate::api::Review> = HashMap::new();
                for review in &reviews {
                    let dominated = latest_by_user
                        .get(&review.user.login)
                        .map_or(true, |existing| review.submitted_at > existing.submitted_at);
                    if dominated {
                        latest_by_user.insert(review.user.login.clone(), review);
                    }
                }

                let mut sorted: Vec<_> = latest_by_user.values().collect();
                sorted.sort_by_key(|r| r.submitted_at);

                for review in sorted {
                    match review.state.as_str() {
                        "APPROVED" => {
                            println!("      {}", green(&format!("✓ approved by {}", review.user.login)));
                        }
                        "CHANGES_REQUESTED" => {
                            println!("      {}", red(&format!("✗ changes requested by {}", review.user.login)));
                        }
                        "COMMENTED" => {
                            println!("      {}", dim(&format!("💬 review comment by {}", review.user.login)));
                        }
                        _ => {} // Skip PENDING, DISMISSED
                    }
                }
            }
        }

        print_comment_info(client, n);
        println!("      {}", dim(&time_ago(&n.updated_at)));
        println!();
    }
    Ok(())
}

pub fn threads(client: &Client, all: bool) -> Result<()> {
    header("── Threads ───────────────────────────────────");
    let notes = client.notifications(all)?;
    let hits: Vec<_> = notes.iter().filter(|n| n.reason == "comment").collect();

    if hits.is_empty() {
        println!("  {}", green("✓ No updates on your threads."));
    } else {
        println!("  {} thread update(s)\n", hits.len());
        for n in hits {
            let icon = if n.subject.kind == "PullRequest" { "⎇" } else { "◉" };
            println!("  {icon}  {}  {}", cyan(&n.repository.full_name), n.subject.title);
            print_comment_info(client, n);
            println!("      {}", dim(&time_ago(&n.updated_at)));
            println!();
        }
    }

    // Resolved review threads (from GraphQL — not covered by notifications)
    match client.resolved_threads() {
        Ok(resolved) if !resolved.is_empty() => {
            println!("\n  {}\n", dim(&format!("{} resolved comment(s)", resolved.len())));
            for r in &resolved {
                println!("  ⎇  {}  {}", cyan(&r.repo), r.pr_title);
                println!("      {}", dim(&truncate_body(&r.comment_body, 120)));
                println!("      {}", dim(&time_ago(&r.updated_at)));
                println!();
            }
        }
        Err(e) => {
            println!("\n  {}", dim(&format!("Could not fetch resolved threads: {e}")));
        }
        _ => {}
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
