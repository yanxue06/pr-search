use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The state of a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

impl PrState {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Merged => "merged",
        }
    }
}

impl std::fmt::Display for PrState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A review comment on a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub author: String,
    pub body: String,
}

/// All relevant data for a single pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrData {
    /// PR number (e.g., #42)
    pub number: u64,
    /// PR title
    pub title: String,
    /// PR body/description (may be empty)
    pub body: String,
    /// PR author login
    pub author: String,
    /// PR state
    pub state: PrState,
    /// Labels attached to the PR
    pub labels: Vec<String>,
    /// When the PR was created
    pub created_at: DateTime<Utc>,
    /// When the PR was last updated
    pub updated_at: DateTime<Utc>,
    /// When the PR was merged (if merged)
    pub merged_at: Option<DateTime<Utc>>,
    /// When the PR was closed (if closed)
    pub closed_at: Option<DateTime<Utc>>,
    /// URL to the PR on GitHub
    pub html_url: String,
    /// Review comments on the PR
    pub review_comments: Vec<ReviewComment>,
    /// The diff text (if fetched)
    pub diff: Option<String>,
    /// Repository in owner/repo format
    pub repo: String,
}

impl PrData {
    /// Convert PR data to a text representation suitable for embedding.
    /// Concatenates title, body, review comments, and optionally diff.
    pub fn to_embedding_text(&self) -> String {
        let mut parts = Vec::new();

        parts.push(self.title.clone());

        if !self.body.is_empty() {
            parts.push(self.body.clone());
        }

        parts.push(format!("Author: {}", self.author));

        if !self.labels.is_empty() {
            parts.push(format!("Labels: {}", self.labels.join(", ")));
        }

        for comment in &self.review_comments {
            parts.push(format!("{}: {}", comment.author, comment.body));
        }

        if let Some(ref diff) = self.diff {
            // Truncate diff at 10,000 bytes at a valid UTF-8 boundary
            let truncated = truncate_at_utf8_boundary(diff, 10_000);
            parts.push(truncated.to_string());
        }

        parts.join("\n")
    }
}

/// Truncate a string at the given byte limit, ensuring we don't split a UTF-8 character.
fn truncate_at_utf8_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_pr() -> PrData {
        PrData {
            number: 42,
            title: "Fix authentication race condition".into(),
            body: "This PR fixes a race condition in the auth middleware.".into(),
            author: "octocat".into(),
            state: PrState::Merged,
            labels: vec!["bug".into(), "auth".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            merged_at: Some(Utc::now()),
            closed_at: Some(Utc::now()),
            html_url: "https://github.com/owner/repo/pull/42".into(),
            review_comments: vec![ReviewComment {
                author: "reviewer".into(),
                body: "LGTM, good fix for the synchronization issue".into(),
            }],
            diff: Some("+fn lock_session() {\n-fn get_session() {".into()),
            repo: "owner/repo".into(),
        }
    }

    #[test]
    fn test_to_embedding_text_includes_all_fields() {
        let pr = sample_pr();
        let text = pr.to_embedding_text();

        assert!(text.contains("Fix authentication race condition"));
        assert!(text.contains("race condition in the auth middleware"));
        assert!(text.contains("Author: octocat"));
        assert!(text.contains("Labels: bug, auth"));
        assert!(text.contains("LGTM, good fix"));
        assert!(text.contains("+fn lock_session"));
    }

    #[test]
    fn test_to_embedding_text_empty_body() {
        let mut pr = sample_pr();
        pr.body = String::new();
        let text = pr.to_embedding_text();

        // Should not have double newlines from empty body
        assert!(!text.contains("\n\n"));
        assert!(text.contains("Fix authentication"));
    }

    #[test]
    fn test_to_embedding_text_no_labels() {
        let mut pr = sample_pr();
        pr.labels.clear();
        let text = pr.to_embedding_text();
        assert!(!text.contains("Labels:"));
    }

    #[test]
    fn test_to_embedding_text_no_diff() {
        let mut pr = sample_pr();
        pr.diff = None;
        let text = pr.to_embedding_text();
        assert!(!text.contains("lock_session"));
    }

    #[test]
    fn test_to_embedding_text_no_review_comments() {
        let mut pr = sample_pr();
        pr.review_comments.clear();
        let text = pr.to_embedding_text();
        assert!(!text.contains("LGTM"));
    }

    #[test]
    fn test_truncate_at_utf8_boundary_ascii() {
        let s = "hello world";
        assert_eq!(truncate_at_utf8_boundary(s, 5), "hello");
    }

    #[test]
    fn test_truncate_at_utf8_boundary_multibyte() {
        let s = "hello 世界";
        // '世' starts at byte 6, is 3 bytes. Truncating at 7 should back up to 6.
        assert_eq!(truncate_at_utf8_boundary(s, 7), "hello ");
    }

    #[test]
    fn test_truncate_at_utf8_boundary_no_truncation() {
        let s = "short";
        assert_eq!(truncate_at_utf8_boundary(s, 100), "short");
    }

    #[test]
    fn test_pr_state_display() {
        assert_eq!(PrState::Open.to_string(), "open");
        assert_eq!(PrState::Closed.to_string(), "closed");
        assert_eq!(PrState::Merged.to_string(), "merged");
    }

    #[test]
    fn test_pr_data_serialization_roundtrip() {
        let pr = sample_pr();
        let json = serde_json::to_string(&pr).unwrap();
        let deserialized: PrData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.number, 42);
        assert_eq!(deserialized.title, "Fix authentication race condition");
        assert_eq!(deserialized.author, "octocat");
        assert_eq!(deserialized.state, PrState::Merged);
        assert_eq!(deserialized.labels, vec!["bug", "auth"]);
        assert_eq!(deserialized.review_comments.len(), 1);
    }

    #[test]
    fn test_diff_truncation_in_embedding() {
        let mut pr = sample_pr();
        // Create a diff larger than 10,000 bytes
        pr.diff = Some("x".repeat(15_000));
        let text = pr.to_embedding_text();

        // The diff portion should be truncated to ~10,000 bytes
        // Total text includes title, body, author, labels, comments, then truncated diff
        let diff_line = text.lines().last().unwrap();
        assert_eq!(diff_line.len(), 10_000);
    }
}
