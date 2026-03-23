mod builder;
mod storage;

pub use builder::IndexBuilder;
pub use storage::IndexStorage;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::github::PrData;

/// Current index format version. Bump when the format changes.
pub const INDEX_VERSION: u32 = 1;

/// A single entry in the semantic index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// The PR data
    pub pr: PrData,
    /// The embedding vector (384 dimensions, L2-normalized)
    pub embedding: Vec<f32>,
}

/// The complete semantic index for a repository's PRs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticIndex {
    /// Index format version
    pub version: u32,
    /// Repository in owner/repo format
    pub repo: String,
    /// When this index was created
    pub created_at: DateTime<Utc>,
    /// When this index was last updated
    pub updated_at: DateTime<Utc>,
    /// The highest PR number in the index (for incremental updates)
    pub last_pr_number: u64,
    /// Whether diffs were included during indexing
    pub with_diffs: bool,
    /// All indexed entries
    pub entries: Vec<IndexEntry>,
}

impl SemanticIndex {
    /// Create a new empty index for a repository.
    pub fn new(repo: &str, with_diffs: bool) -> Self {
        let now = Utc::now();
        Self {
            version: INDEX_VERSION,
            repo: repo.to_string(),
            created_at: now,
            updated_at: now,
            last_pr_number: 0,
            with_diffs,
            entries: Vec::new(),
        }
    }

    /// Number of indexed PRs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Add an entry to the index.
    pub fn add_entry(&mut self, entry: IndexEntry) {
        if entry.pr.number > self.last_pr_number {
            self.last_pr_number = entry.pr.number;
        }
        self.updated_at = Utc::now();
        self.entries.push(entry);
    }

    /// Get PR numbers that are already indexed.
    pub fn indexed_pr_numbers(&self) -> Vec<u64> {
        self.entries.iter().map(|e| e.pr.number).collect()
    }

    /// Remove duplicate entries (keep the latest by PR number).
    pub fn deduplicate(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.entries.retain(|e| seen.insert(e.pr.number));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{PrData, PrState, ReviewComment};

    fn sample_entry(number: u64) -> IndexEntry {
        IndexEntry {
            pr: PrData {
                number,
                title: format!("PR #{number}"),
                body: "Test body".into(),
                author: "test".into(),
                state: PrState::Open,
                labels: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: None,
                closed_at: None,
                html_url: format!("https://github.com/o/r/pull/{number}"),
                review_comments: vec![],
                diff: None,
                repo: "o/r".into(),
            },
            embedding: vec![0.0; 384],
        }
    }

    #[test]
    fn test_new_index() {
        let idx = SemanticIndex::new("owner/repo", false);
        assert_eq!(idx.version, INDEX_VERSION);
        assert_eq!(idx.repo, "owner/repo");
        assert!(!idx.with_diffs);
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        assert_eq!(idx.last_pr_number, 0);
    }

    #[test]
    fn test_add_entry_updates_last_pr_number() {
        let mut idx = SemanticIndex::new("o/r", false);
        idx.add_entry(sample_entry(5));
        assert_eq!(idx.last_pr_number, 5);
        assert_eq!(idx.len(), 1);

        idx.add_entry(sample_entry(10));
        assert_eq!(idx.last_pr_number, 10);
        assert_eq!(idx.len(), 2);

        // Adding a lower number shouldn't decrease last_pr_number
        idx.add_entry(sample_entry(3));
        assert_eq!(idx.last_pr_number, 10);
        assert_eq!(idx.len(), 3);
    }

    #[test]
    fn test_indexed_pr_numbers() {
        let mut idx = SemanticIndex::new("o/r", false);
        idx.add_entry(sample_entry(1));
        idx.add_entry(sample_entry(5));
        idx.add_entry(sample_entry(3));

        let numbers = idx.indexed_pr_numbers();
        assert_eq!(numbers, vec![1, 5, 3]);
    }

    #[test]
    fn test_deduplicate() {
        let mut idx = SemanticIndex::new("o/r", false);
        idx.add_entry(sample_entry(1));
        idx.add_entry(sample_entry(2));
        idx.add_entry(sample_entry(1)); // duplicate
        idx.add_entry(sample_entry(3));
        idx.add_entry(sample_entry(2)); // duplicate

        assert_eq!(idx.len(), 5);
        idx.deduplicate();
        assert_eq!(idx.len(), 3);
        assert_eq!(idx.indexed_pr_numbers(), vec![1, 2, 3]);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut idx = SemanticIndex::new("owner/repo", true);
        idx.add_entry(IndexEntry {
            pr: PrData {
                number: 42,
                title: "Test PR".into(),
                body: "Body".into(),
                author: "dev".into(),
                state: PrState::Merged,
                labels: vec!["bug".into()],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: Some(Utc::now()),
                closed_at: Some(Utc::now()),
                html_url: "https://github.com/o/r/pull/42".into(),
                review_comments: vec![ReviewComment {
                    author: "reviewer".into(),
                    body: "LGTM".into(),
                }],
                diff: Some("+new line".into()),
                repo: "owner/repo".into(),
            },
            embedding: vec![0.1; 384],
        });

        let bytes = bincode::serialize(&idx).unwrap();
        let deserialized: SemanticIndex = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.version, INDEX_VERSION);
        assert_eq!(deserialized.repo, "owner/repo");
        assert!(deserialized.with_diffs);
        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized.entries[0].pr.number, 42);
        assert_eq!(deserialized.entries[0].pr.title, "Test PR");
        assert_eq!(deserialized.entries[0].embedding.len(), 384);
    }

    #[test]
    fn test_large_index_roundtrip() {
        let mut idx = SemanticIndex::new("o/r", false);
        for i in 1..=100 {
            idx.add_entry(sample_entry(i));
        }

        let bytes = bincode::serialize(&idx).unwrap();
        let deserialized: SemanticIndex = bincode::deserialize(&bytes).unwrap();
        assert_eq!(deserialized.len(), 100);
        assert_eq!(deserialized.last_pr_number, 100);
    }
}
