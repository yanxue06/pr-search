use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::process::Command;

use crate::errors::GitHubError;

use super::model::{PrData, PrState, ReviewComment};

/// Fetches PR data from GitHub using the `gh` CLI.
pub struct GitHubFetcher {
    owner: String,
    repo: String,
}

impl GitHubFetcher {
    /// Create a new fetcher for the given repository.
    ///
    /// `repo_spec` should be in "owner/repo" format.
    pub fn new(repo_spec: &str) -> Result<Self> {
        let parts: Vec<&str> = repo_spec.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            anyhow::bail!("Repository must be in owner/repo format, got: {repo_spec}");
        }

        Ok(Self {
            owner: parts[0].to_string(),
            repo: parts[1].to_string(),
        })
    }

    /// Check that the `gh` CLI is installed and authenticated.
    pub fn check_prerequisites(&self) -> Result<()> {
        // Check gh is installed
        let output = Command::new("gh").arg("--version").output();
        match output {
            Err(_) => return Err(GitHubError::GhNotInstalled.into()),
            Ok(o) if !o.status.success() => return Err(GitHubError::GhNotInstalled.into()),
            _ => {}
        }

        // Check authentication
        let output = Command::new("gh")
            .args(["auth", "status"])
            .output()
            .context("Failed to check gh auth status")?;

        if !output.status.success() {
            return Err(GitHubError::NotAuthenticated.into());
        }

        Ok(())
    }

    /// Fetch all PRs (or up to `limit`) from the repository.
    pub fn fetch_prs(&self, limit: Option<usize>, with_diffs: bool) -> Result<Vec<PrData>> {
        self.check_prerequisites()?;

        let limit_str = limit
            .map(|n| n.to_string())
            .unwrap_or_else(|| "1000".to_string());

        tracing::info!(
            owner = %self.owner,
            repo = %self.repo,
            limit = %limit_str,
            "Fetching PRs"
        );

        // Use gh to list PRs with JSON output
        let output = Command::new("gh")
            .args([
                "pr",
                "list",
                "--repo",
                &format!("{}/{}", self.owner, self.repo),
                "--state",
                "all",
                "--limit",
                &limit_str,
                "--json",
                "number,title,body,author,state,labels,createdAt,updatedAt,mergedAt,closedAt,url,reviewDecision",
            ])
            .output()
            .context("Failed to execute gh pr list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Could not resolve to a Repository") {
                return Err(GitHubError::RepoNotFound {
                    owner: self.owner.clone(),
                    repo: self.repo.clone(),
                }
                .into());
            }
            if stderr.contains("rate limit") || stderr.contains("API rate limit") {
                return Err(GitHubError::RateLimited {
                    reset_at: "unknown".into(),
                }
                .into());
            }
            return Err(GitHubError::FetchFailed {
                message: stderr.to_string(),
            }
            .into());
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let raw_prs: Vec<serde_json::Value> =
            serde_json::from_str(&json_str).map_err(|e| GitHubError::ParseError {
                message: e.to_string(),
            })?;

        if raw_prs.is_empty() {
            return Err(GitHubError::NoPrsFound {
                owner: self.owner.clone(),
                repo: self.repo.clone(),
            }
            .into());
        }

        let pb = ProgressBar::new(raw_prs.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} PRs ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );

        let mut prs = Vec::with_capacity(raw_prs.len());

        for raw in &raw_prs {
            let pr = self.parse_pr(raw, with_diffs)?;
            prs.push(pr);
            pb.inc(1);
        }

        pb.finish_with_message("Done fetching PRs");
        Ok(prs)
    }

    /// Parse a single PR from the gh JSON output.
    fn parse_pr(&self, raw: &serde_json::Value, with_diffs: bool) -> Result<PrData> {
        let number = raw["number"].as_u64().unwrap_or(0);

        let state = match raw["state"].as_str().unwrap_or("") {
            "OPEN" => PrState::Open,
            "CLOSED" => {
                // gh reports CLOSED for both closed and merged; check mergedAt
                if raw["mergedAt"].is_string() {
                    PrState::Merged
                } else {
                    PrState::Closed
                }
            }
            "MERGED" => PrState::Merged,
            other => {
                tracing::warn!(
                    state = other,
                    number,
                    "Unknown PR state, defaulting to Open"
                );
                PrState::Open
            }
        };

        let labels: Vec<String> = raw["labels"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| l["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let parse_dt = |key: &str| -> Option<chrono::DateTime<chrono::Utc>> {
            raw[key]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc))
        };

        // Fetch review comments for this PR
        let review_comments = self.fetch_review_comments(number)?;

        // Optionally fetch diff
        let diff = if with_diffs {
            self.fetch_diff(number).ok()
        } else {
            None
        };

        Ok(PrData {
            number,
            title: raw["title"].as_str().unwrap_or("").to_string(),
            body: raw["body"].as_str().unwrap_or("").to_string(),
            author: raw["author"]["login"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            state,
            labels,
            created_at: parse_dt("createdAt").unwrap_or_else(chrono::Utc::now),
            updated_at: parse_dt("updatedAt").unwrap_or_else(chrono::Utc::now),
            merged_at: parse_dt("mergedAt"),
            closed_at: parse_dt("closedAt"),
            html_url: raw["url"].as_str().unwrap_or("").to_string(),
            review_comments,
            diff,
            repo: format!("{}/{}", self.owner, self.repo),
        })
    }

    /// Fetch review comments for a specific PR.
    fn fetch_review_comments(&self, pr_number: u64) -> Result<Vec<ReviewComment>> {
        let output = Command::new("gh")
            .args([
                "api",
                &format!(
                    "repos/{}/{}/pulls/{}/comments",
                    self.owner, self.repo, pr_number
                ),
                "--jq",
                ".[] | {author: .user.login, body: .body}",
            ])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let mut comments = Vec::new();

                for line in stdout.lines() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                        comments.push(ReviewComment {
                            author: val["author"].as_str().unwrap_or("unknown").to_string(),
                            body: val["body"].as_str().unwrap_or("").to_string(),
                        });
                    }
                }

                Ok(comments)
            }
            _ => {
                tracing::debug!(pr_number, "Failed to fetch review comments, continuing");
                Ok(vec![])
            }
        }
    }

    /// Fetch the diff for a specific PR.
    fn fetch_diff(&self, pr_number: u64) -> Result<String> {
        let output = Command::new("gh")
            .args([
                "pr",
                "diff",
                &pr_number.to_string(),
                "--repo",
                &format!("{}/{}", self.owner, self.repo),
            ])
            .output()
            .context("Failed to fetch PR diff")?;

        if !output.status.success() {
            anyhow::bail!("Failed to fetch diff for PR #{pr_number}");
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn repo_spec(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_valid_repo() {
        let fetcher = GitHubFetcher::new("octocat/hello-world").unwrap();
        assert_eq!(fetcher.owner(), "octocat");
        assert_eq!(fetcher.repo(), "hello-world");
        assert_eq!(fetcher.repo_spec(), "octocat/hello-world");
    }

    #[test]
    fn test_new_invalid_repo_no_slash() {
        let result = GitHubFetcher::new("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_new_invalid_repo_empty_parts() {
        assert!(GitHubFetcher::new("/repo").is_err());
        assert!(GitHubFetcher::new("owner/").is_err());
        assert!(GitHubFetcher::new("/").is_err());
    }

    #[test]
    fn test_new_repo_with_nested_slash() {
        // "org/sub/repo" — owner="org", repo="sub/repo" (splitn with n=2)
        let fetcher = GitHubFetcher::new("org/sub/repo").unwrap();
        assert_eq!(fetcher.owner(), "org");
        assert_eq!(fetcher.repo(), "sub/repo");
    }

    #[test]
    fn test_parse_pr_basic() {
        let fetcher = GitHubFetcher::new("octocat/hello-world").unwrap();
        let raw = serde_json::json!({
            "number": 1,
            "title": "Add README",
            "body": "This adds a README file",
            "author": {"login": "octocat"},
            "state": "MERGED",
            "labels": [{"name": "documentation"}],
            "createdAt": "2025-01-01T00:00:00Z",
            "updatedAt": "2025-01-02T00:00:00Z",
            "mergedAt": "2025-01-02T00:00:00Z",
            "closedAt": "2025-01-02T00:00:00Z",
            "url": "https://github.com/octocat/hello-world/pull/1"
        });

        let pr = fetcher.parse_pr(&raw, false).unwrap();
        assert_eq!(pr.number, 1);
        assert_eq!(pr.title, "Add README");
        assert_eq!(pr.author, "octocat");
        assert_eq!(pr.state, PrState::Merged);
        assert_eq!(pr.labels, vec!["documentation"]);
        assert!(pr.merged_at.is_some());
        assert!(pr.diff.is_none());
    }

    #[test]
    fn test_parse_pr_closed_not_merged() {
        let fetcher = GitHubFetcher::new("o/r").unwrap();
        let raw = serde_json::json!({
            "number": 2,
            "title": "Rejected PR",
            "body": "",
            "author": {"login": "user"},
            "state": "CLOSED",
            "labels": [],
            "createdAt": "2025-01-01T00:00:00Z",
            "updatedAt": "2025-01-01T00:00:00Z",
            "url": "https://github.com/o/r/pull/2"
        });

        let pr = fetcher.parse_pr(&raw, false).unwrap();
        assert_eq!(pr.state, PrState::Closed);
        assert!(pr.merged_at.is_none());
    }

    #[test]
    fn test_parse_pr_open() {
        let fetcher = GitHubFetcher::new("o/r").unwrap();
        let raw = serde_json::json!({
            "number": 3,
            "title": "WIP feature",
            "body": "Work in progress",
            "author": {"login": "dev"},
            "state": "OPEN",
            "labels": [{"name": "WIP"}, {"name": "feature"}],
            "createdAt": "2025-06-01T00:00:00Z",
            "updatedAt": "2025-06-15T00:00:00Z",
            "url": "https://github.com/o/r/pull/3"
        });

        let pr = fetcher.parse_pr(&raw, false).unwrap();
        assert_eq!(pr.state, PrState::Open);
        assert_eq!(pr.labels, vec!["WIP", "feature"]);
        assert!(pr.merged_at.is_none());
        assert!(pr.closed_at.is_none());
    }
}
