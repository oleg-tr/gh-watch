# ghw - GitHub notifications without the noise

A terminal dashboard for GitHub notifications. Shows @mentions, activity on your PRs, and event feeds from repos you watch.

## 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts (default installation is fine), then reload your shell:

```bash
source "$HOME/.cargo/env"
```

Verify:

```bash
rustc --version
cargo --version
```

## 2. Build & Install

```bash
git clone <repo-url> && cd gh-watch
cargo install --path .
```

This places the `ghw` binary in `~/.cargo/bin/` (should already be on your `$PATH` after Rust install).

Alternatively, build without installing:

```bash
cargo build --release
# binary at ./target/release/ghw
```

## 3. Create a GitHub Token

1. Go to **GitHub > Settings > Developer settings > Personal access tokens > Tokens (classic)**
2. Click **Generate new token (classic)**
3. Set a name (e.g. `ghw-cli`) and expiration
4. Select scopes:
   - `notifications`
   - `repo`
5. Click **Generate token** and copy it

### Authorize SSO (required for organization repos)

If your organization uses SAML SSO (e.g. kununu), the token won't work for org repos until you authorize it:

1. Go to **Settings > Developer settings > Personal access tokens > Tokens (classic)**
2. Find your token and click **Configure SSO**
3. Click **Authorize** next to your organization
4. Complete the SSO authentication if prompted

Without this step, API calls to organization repositories will return 404.

## 4. Configure the Token

**Option A** - environment variable:

```bash
export GITHUB_TOKEN=ghp_yourTokenHere
```

Add to your `~/.zshrc` or `~/.bashrc` to persist across sessions.

**Option B** - token file:

```bash
mkdir -p ~/.config/ghw
echo "ghp_yourTokenHere" > ~/.config/ghw/token
chmod 600 ~/.config/ghw/token
```

## Usage

```
ghw                  # full dashboard (mentions + PRs + feed)
ghw mentions         # PRs/issues where you were @mentioned
ghw my-prs           # reviews and comments on your own PRs
ghw feed             # recent activity in watched repos
ghw watch owner/repo # add a repo to your watch list
ghw unwatch owner/repo
ghw watched          # list watched repos
```

### Flags

| Flag | Commands | Description |
|------|----------|-------------|
| `-a, --all` | `mentions`, `my-prs` | Include already-read notifications |
| `-l, --limit N` | `feed` | Number of events per repo (default: 10) |

## Watch List

Watched repos are stored in `~/.ghw-repos` (one `owner/repo` per line). Manage with `ghw watch` / `ghw unwatch`, or edit the file directly.
