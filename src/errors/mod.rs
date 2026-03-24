mod embedding;
mod github;
mod index;
mod search;

pub use embedding::EmbeddingError;
pub use github::GitHubError;
pub use index::IndexError;
pub use search::SearchError;

/// Trait for domain errors that provide actionable hints and stable error codes.
pub trait DomainError: std::error::Error {
    /// Returns a user-friendly hint on how to fix the error.
    fn hint(&self) -> &str;

    /// Returns a stable error code (e.g., "E1001") for programmatic use.
    fn code(&self) -> &str;
}

/// Sanitize file paths in error messages to avoid leaking usernames.
pub fn sanitize_path(msg: &str) -> String {
    let mut result = msg.to_string();

    if let Ok(home) = std::env::var("HOME") {
        result = result.replace(&home, "~");
    }

    // Strip query parameters from URLs to avoid leaking tokens
    if let Some(idx) = result.find("?") {
        if result[..idx].contains("http") {
            result = format!("{}?<redacted>", &result[..idx]);
        }
    }

    result
}

/// Format an error chain for display to the user.
pub fn format_error_chain(err: &anyhow::Error) -> String {
    let mut output = String::new();
    output.push_str(&format!("Error: {}\n", sanitize_path(&err.to_string())));

    for (i, cause) in err.chain().skip(1).enumerate() {
        output.push_str(&format!(
            "  {}. {}\n",
            i + 1,
            sanitize_path(&cause.to_string())
        ));
    }

    // Check if any error in the chain implements DomainError
    for cause in err.chain() {
        if let Some(domain_err) = cause.downcast_ref::<GitHubError>() {
            append_domain_info(&mut output, domain_err);
            break;
        } else if let Some(domain_err) = cause.downcast_ref::<EmbeddingError>() {
            append_domain_info(&mut output, domain_err);
            break;
        } else if let Some(domain_err) = cause.downcast_ref::<IndexError>() {
            append_domain_info(&mut output, domain_err);
            break;
        } else if let Some(domain_err) = cause.downcast_ref::<SearchError>() {
            append_domain_info(&mut output, domain_err);
            break;
        }
    }

    output
}

fn append_domain_info(output: &mut String, err: &dyn DomainError) {
    output.push_str(&format!("\n  Hint: {}\n", err.hint()));
    output.push_str(&format!("  Code: {}\n", err.code()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_home_path() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/testuser".to_string());
        let msg = format!("File not found: {}/projects/test.rs", home);
        let sanitized = sanitize_path(&msg);
        assert!(sanitized.contains("~/projects/test.rs"));
        assert!(!sanitized.contains(&home));
    }

    #[test]
    fn test_sanitize_url_query_params() {
        let msg = "Failed to download from https://example.com/model?token=secret123&v=2";
        let sanitized = sanitize_path(msg);
        assert!(sanitized.contains("https://example.com/model?<redacted>"));
        assert!(!sanitized.contains("secret123"));
    }

    #[test]
    fn test_sanitize_no_url_question_mark() {
        let msg = "What is this? A bug?";
        let sanitized = sanitize_path(msg);
        assert_eq!(sanitized, msg);
    }

    #[test]
    fn test_format_error_chain_with_github_error() {
        let err = GitHubError::NotAuthenticated;
        let anyhow_err: anyhow::Error = err.into();
        let formatted = format_error_chain(&anyhow_err);
        assert!(formatted.contains("Hint:"));
        assert!(formatted.contains("Code:"));
        assert!(formatted.contains("E2"));
    }
}
