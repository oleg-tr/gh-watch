use anyhow::{anyhow, Context, Result};
use reqwest::blocking;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use chrono::{DateTime, Utc};

const BASE: &str = "https://api.github.com";

// ── Models ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Notification {
    pub id: String,
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
    pub url: Option<String>,
    pub latest_comment_url: Option<String>,
}

#[derive(Deserialize)]
pub struct Comment {
    pub user: Actor,
    pub body: String,
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

#[derive(Deserialize)]
pub struct Review {
    pub user: Actor,
    pub state: String,
    pub submitted_at: DateTime<Utc>,
}

pub struct ResolvedThread {
    pub repo: String,
    pub pr_title: String,
    pub comment_body: String,
    pub updated_at: DateTime<Utc>,
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

    pub fn mark_thread_read(&self, thread_id: &str) -> Result<()> {
        let resp = self.inner
            .patch(&format!("{BASE}/notifications/threads/{thread_id}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .context("Failed to mark thread as read")?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to mark thread {thread_id} as read: HTTP {}", resp.status().as_u16()));
        }
        Ok(())
    }

    pub fn events(&self, repo: &str, limit: usize) -> Result<Vec<Event>> {
        let per = limit.min(100).to_string();
        self.get(
            &format!("{BASE}/repos/{repo}/events"),
            &[("per_page", per.as_str())],
        )
    }

    pub fn comment(&self, url: &str) -> Result<Comment> {
        self.get(url, &[])
    }

    pub fn reviews(&self, pr_api_url: &str) -> Result<Vec<Review>> {
        self.get(&format!("{pr_api_url}/reviews"), &[])
    }

    /// Just checks if a repo is accessible — used when adding to watch list.
    pub fn repo_exists(&self, repo: &str) -> bool {
        self.get::<serde_json::Value>(&format!("{BASE}/repos/{repo}"), &[]).is_ok()
    }

    fn graphql(&self, query: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({ "query": query });
        let resp = self.inner
            .post("https://api.github.com/graphql")
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&body)
            .send()
            .context("GraphQL request failed")?;

        match resp.status().as_u16() {
            401 => return Err(anyhow!("Bad token — check your GITHUB_TOKEN.")),
            s if s >= 400 => return Err(anyhow!("GitHub GraphQL error: HTTP {s}")),
            _ => {}
        }

        let json: serde_json::Value = resp.json()?;
        if let Some(errors) = json.get("errors") {
            return Err(anyhow!("GraphQL errors: {errors}"));
        }
        Ok(json)
    }

    pub fn viewer_login(&self) -> Result<String> {
        let json = self.graphql("{ viewer { login } }")?;
        json["data"]["viewer"]["login"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Could not determine your GitHub username"))
    }

    pub fn resolved_threads(&self) -> Result<Vec<ResolvedThread>> {
        let login = self.viewer_login()?;
        let since = (Utc::now() - chrono::Duration::days(7)).format("%Y-%m-%d");
        let query = format!(r#"
        {{
          search(query: "reviewed-by:{login} is:pr updated:>={since}", type: ISSUE, first: 30) {{
            nodes {{
              ... on PullRequest {{
                title
                updatedAt
                repository {{ nameWithOwner }}
                reviewThreads(first: 100) {{
                  nodes {{
                    isResolved
                    comments(first: 10) {{
                      nodes {{
                        author {{ login }}
                        body
                        createdAt
                      }}
                    }}
                  }}
                }}
              }}
            }}
          }}
        }}
        "#);

        let json = self.graphql(&query)?;
        let mut results = Vec::new();
        let empty = vec![];

        let nodes = json["data"]["search"]["nodes"]
            .as_array()
            .unwrap_or(&empty);

        for pr in nodes {
            let repo = pr["repository"]["nameWithOwner"].as_str().unwrap_or("");
            let title = pr["title"].as_str().unwrap_or("");
            let updated = pr["updatedAt"].as_str().unwrap_or("");

            let thread_nodes = pr["reviewThreads"]["nodes"]
                .as_array()
                .unwrap_or(&empty);

            for thread in thread_nodes {
                if thread["isResolved"].as_bool() != Some(true) {
                    continue;
                }

                let comments = thread["comments"]["nodes"]
                    .as_array()
                    .unwrap_or(&empty);

                let user_comment = comments.iter().find(|c| {
                    c["author"]["login"].as_str() == Some(&login)
                });

                if let Some(comment) = user_comment {
                    let body = comment["body"].as_str().unwrap_or("").to_string();
                    let parsed_date = updated.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now());

                    results.push(ResolvedThread {
                        repo: repo.to_string(),
                        pr_title: title.to_string(),
                        comment_body: body,
                        updated_at: parsed_date,
                    });
                }
            }
        }

        Ok(results)
    }
}

fn token_from_file() -> Result<String, std::env::VarError> {
    dirs::config_dir()
        .and_then(|p| std::fs::read_to_string(p.join("ghw/token")).ok())
        .map(|s| s.trim().to_string())
        .ok_or(std::env::VarError::NotPresent)
}
