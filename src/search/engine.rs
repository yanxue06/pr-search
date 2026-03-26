use anyhow::Result;

use crate::errors::SearchError;
use crate::index::SemanticIndex;

use super::filter::SearchFilter;

/// A single search result with similarity score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The PR number
    pub number: u64,
    /// The PR title
    pub title: String,
    /// The PR author
    pub author: String,
    /// The PR state (open/closed/merged)
    pub state: String,
    /// URL to the PR on GitHub
    pub html_url: String,
    /// Labels on the PR
    pub labels: Vec<String>,
    /// When the PR was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Cosine similarity score (0.0 to 1.0 for normalized vectors)
    pub score: f32,
}

/// Search engine that performs semantic search over an index.
pub struct SearchEngine;

impl SearchEngine {
    /// Search the index using a query embedding.
    ///
    /// Returns results sorted by similarity score (highest first).
    pub fn search(
        index: &SemanticIndex,
        query_embedding: &[f32],
        filter: &SearchFilter,
        num_results: usize,
    ) -> Result<Vec<SearchResult>> {
        if query_embedding.is_empty() {
            return Err(SearchError::EmptyQuery.into());
        }

        if index.is_empty() {
            return Err(crate::errors::IndexError::Empty.into());
        }

        let mut results: Vec<SearchResult> = index
            .entries
            .iter()
            .filter(|entry| filter.matches(&entry.pr))
            .map(|entry| {
                let score = cosine_similarity(query_embedding, &entry.embedding);
                SearchResult {
                    number: entry.pr.number,
                    title: entry.pr.title.clone(),
                    author: entry.pr.author.clone(),
                    state: entry.pr.state.to_string(),
                    html_url: entry.pr.html_url.clone(),
                    labels: entry.pr.labels.clone(),
                    created_at: entry.pr.created_at,
                    score,
                }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to requested number
        results.truncate(num_results);

        Ok(results)
    }
}

/// Compute cosine similarity between two vectors.
///
/// For L2-normalized vectors, this is equivalent to the dot product.
/// We still compute the full formula for safety against non-normalized vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{PrData, PrState};
    use crate::index::{IndexEntry, SemanticIndex};
    use chrono::Utc;

    fn sample_index() -> SemanticIndex {
        let mut idx = SemanticIndex::new("test/repo", false);

        // PR 1: embedding points in direction [1, 0, 0, ...]
        let mut emb1 = vec![0.0; 384];
        emb1[0] = 1.0;
        idx.add_entry(IndexEntry {
            pr: PrData {
                number: 1,
                title: "Fix authentication bug".into(),
                body: "Fixed a race condition".into(),
                author: "alice".into(),
                state: PrState::Merged,
                labels: vec!["bug".into()],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: Some(Utc::now()),
                closed_at: Some(Utc::now()),
                html_url: "https://github.com/test/repo/pull/1".into(),
                review_comments: vec![],
                diff: None,
                repo: "test/repo".into(),
            },
            embedding: emb1,
        });

        // PR 2: embedding points in direction [0, 1, 0, ...]
        let mut emb2 = vec![0.0; 384];
        emb2[1] = 1.0;
        idx.add_entry(IndexEntry {
            pr: PrData {
                number: 2,
                title: "Add new feature".into(),
                body: "New dashboard widget".into(),
                author: "bob".into(),
                state: PrState::Open,
                labels: vec!["feature".into()],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: None,
                closed_at: None,
                html_url: "https://github.com/test/repo/pull/2".into(),
                review_comments: vec![],
                diff: None,
                repo: "test/repo".into(),
            },
            embedding: emb2,
        });

        // PR 3: embedding similar to PR 1
        let mut emb3 = vec![0.0; 384];
        emb3[0] = 0.9;
        emb3[1] = 0.1;
        // Normalize
        let norm = (0.9_f32 * 0.9 + 0.1 * 0.1).sqrt();
        emb3[0] /= norm;
        emb3[1] /= norm;
        idx.add_entry(IndexEntry {
            pr: PrData {
                number: 3,
                title: "Fix session timeout".into(),
                body: "Session handling fix".into(),
                author: "alice".into(),
                state: PrState::Merged,
                labels: vec!["bug".into(), "auth".into()],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: Some(Utc::now()),
                closed_at: Some(Utc::now()),
                html_url: "https://github.com/test/repo/pull/3".into(),
                review_comments: vec![],
                diff: None,
                repo: "test/repo".into(),
            },
            embedding: emb3,
        });

        idx
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_normalized_384_dim() {
        let mut a = vec![0.0; 384];
        let mut b = vec![0.0; 384];
        a[0] = 1.0;
        b[0] = 0.7071;
        b[1] = 0.7071;
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.7071).abs() < 0.001);
    }

    #[test]
    fn test_search_returns_ranked_results() {
        let idx = sample_index();

        // Query similar to PR 1's embedding direction
        let mut query = vec![0.0; 384];
        query[0] = 1.0;

        let results = SearchEngine::search(&idx, &query, &SearchFilter::default(), 10).unwrap();

        assert_eq!(results.len(), 3);
        // PR 1 should be first (identical direction)
        assert_eq!(results[0].number, 1);
        // PR 3 should be second (similar direction)
        assert_eq!(results[1].number, 3);
        // PR 2 should be last (orthogonal)
        assert_eq!(results[2].number, 2);

        assert!(results[0].score > results[1].score);
        assert!(results[1].score > results[2].score);
    }

    #[test]
    fn test_search_respects_num_results() {
        let idx = sample_index();
        let mut query = vec![0.0; 384];
        query[0] = 1.0;

        let results = SearchEngine::search(&idx, &query, &SearchFilter::default(), 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_with_author_filter() {
        let idx = sample_index();
        let mut query = vec![0.0; 384];
        query[0] = 1.0;

        let filter = SearchFilter {
            author: Some("bob".into()),
            ..Default::default()
        };

        let results = SearchEngine::search(&idx, &query, &filter, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].author, "bob");
    }

    #[test]
    fn test_search_empty_index() {
        let idx = SemanticIndex::new("o/r", false);
        let query = vec![1.0; 384];

        let result = SearchEngine::search(&idx, &query, &SearchFilter::default(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_empty_query() {
        let idx = sample_index();
        let result = SearchEngine::search(&idx, &[], &SearchFilter::default(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_result_fields() {
        let idx = sample_index();
        let mut query = vec![0.0; 384];
        query[0] = 1.0;

        let results = SearchEngine::search(&idx, &query, &SearchFilter::default(), 1).unwrap();
        let r = &results[0];

        assert_eq!(r.number, 1);
        assert_eq!(r.title, "Fix authentication bug");
        assert_eq!(r.author, "alice");
        assert_eq!(r.state, "merged");
        assert!(r.html_url.contains("/pull/1"));
        assert_eq!(r.labels, vec!["bug"]);
        assert!(r.score > 0.99);
    }
}
