use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::errors::IndexError;

use super::{SemanticIndex, INDEX_VERSION};

/// Handles reading and writing the semantic index to disk.
pub struct IndexStorage {
    index_path: PathBuf,
}

impl IndexStorage {
    /// Create storage pointing at a specific file path.
    pub fn new(index_path: PathBuf) -> Self {
        Self { index_path }
    }

    /// Create storage for a git repository directory.
    /// The index is stored at `.git/semantic-pr-index` inside the repo.
    pub fn for_repo(repo_path: &Path) -> Result<Self> {
        let git_dir = repo_path.join(".git");

        // Handle git worktrees where .git is a file pointing to the real gitdir
        let actual_git_dir = if git_dir.is_file() {
            let contents = std::fs::read_to_string(&git_dir).context("Failed to read .git file")?;
            let gitdir = contents
                .strip_prefix("gitdir: ")
                .and_then(|s| s.strip_suffix('\n'))
                .ok_or(IndexError::NotInRepo)?;
            PathBuf::from(gitdir)
        } else if git_dir.is_dir() {
            git_dir
        } else {
            return Err(IndexError::NotInRepo.into());
        };

        Ok(Self {
            index_path: actual_git_dir.join("semantic-pr-index"),
        })
    }

    /// Path where the index is stored.
    pub fn path(&self) -> &Path {
        &self.index_path
    }

    /// Check if an index file exists.
    pub fn exists(&self) -> bool {
        self.index_path.exists()
    }

    /// Save the index to disk.
    pub fn save(&self, index: &SemanticIndex) -> Result<()> {
        let bytes = bincode::serialize(index).map_err(|e| IndexError::SerializationFailed {
            message: e.to_string(),
        })?;

        // Write to a temp file first, then rename for atomicity
        let tmp_path = self.index_path.with_extension("tmp");

        if let Some(parent) = self.index_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| IndexError::WriteFailed {
                message: e.to_string(),
            })?;
        }

        std::fs::write(&tmp_path, &bytes).map_err(|e| IndexError::WriteFailed {
            message: e.to_string(),
        })?;

        std::fs::rename(&tmp_path, &self.index_path).map_err(|e| IndexError::WriteFailed {
            message: format!("Failed to finalize index file: {e}"),
        })?;

        tracing::info!(
            path = %self.index_path.display(),
            entries = index.len(),
            size_kb = bytes.len() / 1024,
            "Index saved"
        );

        Ok(())
    }

    /// Load the index from disk.
    pub fn load(&self) -> Result<SemanticIndex> {
        if !self.exists() {
            return Err(IndexError::NotFound {
                path: self.index_path.display().to_string(),
            }
            .into());
        }

        let bytes = std::fs::read(&self.index_path).map_err(|e| IndexError::ReadFailed {
            message: e.to_string(),
        })?;

        let index: SemanticIndex =
            bincode::deserialize(&bytes).map_err(|e| IndexError::DeserializationFailed {
                message: e.to_string(),
            })?;

        // Version check
        if index.version != INDEX_VERSION {
            return Err(IndexError::VersionMismatch {
                expected: INDEX_VERSION,
                found: index.version,
            }
            .into());
        }

        tracing::info!(
            entries = index.len(),
            repo = %index.repo,
            "Index loaded"
        );

        Ok(index)
    }

    /// Delete the index file.
    pub fn delete(&self) -> Result<()> {
        if self.exists() {
            std::fs::remove_file(&self.index_path).map_err(|e| IndexError::WriteFailed {
                message: format!("Failed to delete index: {e}"),
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{PrData, PrState};
    use crate::index::IndexEntry;
    use chrono::Utc;
    use tempfile::TempDir;

    fn sample_index() -> SemanticIndex {
        let mut idx = SemanticIndex::new("test/repo", false);
        idx.add_entry(IndexEntry {
            pr: PrData {
                number: 1,
                title: "Test".into(),
                body: "".into(),
                author: "dev".into(),
                state: PrState::Open,
                labels: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: None,
                closed_at: None,
                html_url: "https://github.com/test/repo/pull/1".into(),
                review_comments: vec![],
                diff: None,
                repo: "test/repo".into(),
            },
            embedding: vec![0.5; 384],
        });
        idx
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = IndexStorage::new(tmp.path().join("test-index"));

        let original = sample_index();
        storage.save(&original).unwrap();

        assert!(storage.exists());

        let loaded = storage.load().unwrap();
        assert_eq!(loaded.repo, "test/repo");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.entries[0].pr.number, 1);
        assert_eq!(loaded.entries[0].embedding.len(), 384);
    }

    #[test]
    fn test_load_nonexistent_returns_error() {
        let storage = IndexStorage::new(PathBuf::from("/nonexistent/index"));
        let result = storage.load();
        assert!(result.is_err());
    }

    #[test]
    fn test_overwrite_existing() {
        let tmp = TempDir::new().unwrap();
        let storage = IndexStorage::new(tmp.path().join("test-index"));

        let idx1 = sample_index();
        storage.save(&idx1).unwrap();

        let mut idx2 = SemanticIndex::new("other/repo", true);
        idx2.add_entry(IndexEntry {
            pr: PrData {
                number: 99,
                title: "Other".into(),
                body: "".into(),
                author: "dev".into(),
                state: PrState::Merged,
                labels: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: Some(Utc::now()),
                closed_at: None,
                html_url: "url".into(),
                review_comments: vec![],
                diff: None,
                repo: "other/repo".into(),
            },
            embedding: vec![0.1; 384],
        });

        storage.save(&idx2).unwrap();

        let loaded = storage.load().unwrap();
        assert_eq!(loaded.repo, "other/repo");
        assert_eq!(loaded.entries[0].pr.number, 99);
    }

    #[test]
    fn test_delete() {
        let tmp = TempDir::new().unwrap();
        let storage = IndexStorage::new(tmp.path().join("test-index"));

        storage.save(&sample_index()).unwrap();
        assert!(storage.exists());

        storage.delete().unwrap();
        assert!(!storage.exists());
    }

    #[test]
    fn test_delete_nonexistent_is_ok() {
        let storage = IndexStorage::new(PathBuf::from("/nonexistent/index"));
        assert!(storage.delete().is_ok());
    }

    #[test]
    fn test_for_repo_not_a_repo() {
        let tmp = TempDir::new().unwrap();
        let result = IndexStorage::for_repo(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_for_repo_with_git_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();

        let storage = IndexStorage::for_repo(tmp.path()).unwrap();
        assert_eq!(
            storage.path(),
            tmp.path().join(".git").join("semantic-pr-index")
        );
    }

    #[test]
    fn test_large_index_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = IndexStorage::new(tmp.path().join("large-index"));

        let mut idx = SemanticIndex::new("big/repo", false);
        for i in 1..=500 {
            idx.add_entry(IndexEntry {
                pr: PrData {
                    number: i,
                    title: format!("PR #{i}"),
                    body: "x".repeat(100),
                    author: "dev".into(),
                    state: PrState::Open,
                    labels: vec![],
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    merged_at: None,
                    closed_at: None,
                    html_url: format!("url/{i}"),
                    review_comments: vec![],
                    diff: None,
                    repo: "big/repo".into(),
                },
                embedding: vec![i as f32 / 500.0; 384],
            });
        }

        storage.save(&idx).unwrap();
        let loaded = storage.load().unwrap();
        assert_eq!(loaded.len(), 500);
        assert_eq!(loaded.last_pr_number, 500);
    }
}
