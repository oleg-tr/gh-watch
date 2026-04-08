use anyhow::{anyhow, Context, Result};
use reqwest::blocking;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use chrono::{DateTime, Utc};

const BASE: &str = "https://api.github.com";

// ── Models ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Notification {
    pub reason: String,
    pub updated_at: DateTime<Utc>,
    pub subject: Subject,
    pub repository: Repository,
}

#[derive(Deserialize)]
pub struct Subject {
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Deserialize)]
pub struct Repository {
    pub full_name: String,
}

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: Actor,
    pub created_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

#[derive(Deserialize)]
pub struct Actor {
    pub login: String,
}

// ── Client ────────────────────────────────────────────────────────────────────

pub struct Client {
    inner: blocking::Client,
    token: String,
}

impl Client {
    pub fn new() -> Result<Self> {
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| token_from_file())
            .map_err(|_| anyhow!(
                "GITHUB_TOKEN not found.\n\
                 Set it via: export GITHUB_TOKEN=<pat>\n\
                 Or save it in ~/.config/ghw/token"
            ))?;

        let inner = blocking::Client::builder()
            .user_agent("ghw-cli/0.1")
            .build()?;

        Ok(Self { inner, token })
    }

    fn get<T: DeserializeOwned>(&self, url: &str, params: &[(&str, &str)]) -> Result<T> {
        let resp = self.inner
            .get(url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .query(params)
            .send()
            .context("Network request failed")?;

        match resp.status().as_u16() {
            401 => return Err(anyhow!("Bad token — check your GITHUB_TOKEN.")),
            404 => return Err(anyhow!("Not found: {url}")),
            s if !resp.status().is_success() => {
                return Err(anyhow!("GitHub API error: HTTP {s}"))
            }
            _ => {}
        }

        Ok(resp.json()?)
    }

    pub fn notifications(&self, include_read: bool) -> Result<Vec<Notification>> {
        let all = if include_read { "true" } else { "false" };
        self.get(&format!("{BASE}/notifications"), &[("all", all), ("per_page", "50")])
    }

    pub fn events(&self, repo: &str, limit: usize) -> Result<Vec<Event>> {
        let per = limit.min(100).to_string();
        self.get(
            &format!("{BASE}/repos/{repo}/events"),
            &[("per_page", per.as_str())],
        )
    }

    /// Just checks if a repo is accessible — used when adding to watch list.
    pub fn repo_exists(&self, repo: &str) -> bool {
        self.get::<serde_json::Value>(&format!("{BASE}/repos/{repo}"), &[]).is_ok()
    }
}

fn token_from_file() -> Result<String, std::env::VarError> {
    dirs::config_dir()
        .and_then(|p| std::fs::read_to_string(p.join("ghw/token")).ok())
        .map(|s| s.trim().to_string())
        .ok_or(std::env::VarError::NotPresent)
}
