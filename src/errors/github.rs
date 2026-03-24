use super::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("GitHub CLI (gh) is not installed")]
    GhNotInstalled,

    #[error("Not authenticated with GitHub CLI")]
    NotAuthenticated,

    #[error("Repository not found: {owner}/{repo}")]
    RepoNotFound { owner: String, repo: String },

    #[error("Rate limit exceeded, resets at {reset_at}")]
    RateLimited { reset_at: String },

    #[error("Failed to fetch PRs: {message}")]
    FetchFailed { message: String },

    #[error("Failed to parse GitHub API response: {message}")]
    ParseError { message: String },

    #[error("No PRs found in repository {owner}/{repo}")]
    NoPrsFound { owner: String, repo: String },
}

impl DomainError for GitHubError {
    fn hint(&self) -> &str {
        match self {
            Self::GhNotInstalled => "Install the GitHub CLI: https://cli.github.com",
            Self::NotAuthenticated => "Run: gh auth login",
            Self::RepoNotFound { .. } => "Check the repository owner and name, or verify your access permissions",
            Self::RateLimited { .. } => "Wait for the rate limit to reset, or authenticate with `gh auth login` for higher limits",
            Self::FetchFailed { .. } => "Check your network connection and try again",
            Self::ParseError { .. } => "This may be a bug. Please report it at https://github.com/yanxue06/pr-search/issues",
            Self::NoPrsFound { .. } => "Verify the repository has pull requests, or check the repository name",
        }
    }

    fn code(&self) -> &str {
        match self {
            Self::GhNotInstalled => "E2001",
            Self::NotAuthenticated => "E2002",
            Self::RepoNotFound { .. } => "E2003",
            Self::RateLimited { .. } => "E2004",
            Self::FetchFailed { .. } => "E2005",
            Self::ParseError { .. } => "E2006",
            Self::NoPrsFound { .. } => "E2007",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = GitHubError::RepoNotFound {
            owner: "octocat".into(),
            repo: "hello-world".into(),
        };
        assert_eq!(err.to_string(), "Repository not found: octocat/hello-world");
    }

    #[test]
    fn test_error_hints_are_actionable() {
        let errors: Vec<GitHubError> = vec![
            GitHubError::GhNotInstalled,
            GitHubError::NotAuthenticated,
            GitHubError::RepoNotFound {
                owner: "a".into(),
                repo: "b".into(),
            },
            GitHubError::RateLimited {
                reset_at: "2026-01-01T00:00:00Z".into(),
            },
            GitHubError::FetchFailed {
                message: "timeout".into(),
            },
            GitHubError::ParseError {
                message: "invalid json".into(),
            },
            GitHubError::NoPrsFound {
                owner: "a".into(),
                repo: "b".into(),
            },
        ];

        for err in &errors {
            assert!(
                !err.hint().is_empty(),
                "Hint should not be empty for {:?}",
                err
            );
            assert!(
                !err.code().is_empty(),
                "Code should not be empty for {:?}",
                err
            );
            assert!(
                err.code().starts_with("E2"),
                "GitHub errors should have E2xxx codes"
            );
        }
    }

    #[test]
    fn test_error_codes_unique() {
        let errors: Vec<GitHubError> = vec![
            GitHubError::GhNotInstalled,
            GitHubError::NotAuthenticated,
            GitHubError::RepoNotFound {
                owner: "a".into(),
                repo: "b".into(),
            },
            GitHubError::RateLimited {
                reset_at: "x".into(),
            },
            GitHubError::FetchFailed {
                message: "x".into(),
            },
            GitHubError::ParseError {
                message: "x".into(),
            },
            GitHubError::NoPrsFound {
                owner: "a".into(),
                repo: "b".into(),
            },
        ];

        let codes: Vec<&str> = errors.iter().map(|e| e.code()).collect();
        let unique: std::collections::HashSet<&&str> = codes.iter().collect();
        assert_eq!(codes.len(), unique.len(), "Error codes must be unique");
    }
}
