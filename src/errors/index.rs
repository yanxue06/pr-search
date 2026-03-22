use super::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Index not found at {path}")]
    NotFound { path: String },

    #[error("Failed to serialize index: {message}")]
    SerializationFailed { message: String },

    #[error("Failed to deserialize index: {message}")]
    DeserializationFailed { message: String },

    #[error("Failed to write index to disk: {message}")]
    WriteFailed { message: String },

    #[error("Failed to read index from disk: {message}")]
    ReadFailed { message: String },

    #[error("Index is empty — no PRs have been indexed")]
    Empty,

    #[error("Index version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },

    #[error("Not in a git repository")]
    NotInRepo,
}

impl DomainError for IndexError {
    fn hint(&self) -> &str {
        match self {
            Self::NotFound { .. } => "Run: pr-search index <owner/repo>",
            Self::SerializationFailed { .. } => {
                "This may be a bug. Please report it at https://github.com/yanxue06/pr-search/issues"
            }
            Self::DeserializationFailed { .. } => {
                "The index may be corrupted. Run: pr-search index --force <owner/repo>"
            }
            Self::WriteFailed { .. } => "Check disk space and file permissions",
            Self::ReadFailed { .. } => "Check file permissions on the .git directory",
            Self::Empty => "Run: pr-search index <owner/repo>",
            Self::VersionMismatch { .. } => "Re-index with: pr-search index --force <owner/repo>",
            Self::NotInRepo => "Navigate to a git repository, or use --path to specify one",
        }
    }

    fn code(&self) -> &str {
        match self {
            Self::NotFound { .. } => "E3001",
            Self::SerializationFailed { .. } => "E3002",
            Self::DeserializationFailed { .. } => "E3003",
            Self::WriteFailed { .. } => "E3004",
            Self::ReadFailed { .. } => "E3005",
            Self::Empty => "E3006",
            Self::VersionMismatch { .. } => "E3007",
            Self::NotInRepo => "E3008",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_errors_have_hints_and_codes() {
        let errors: Vec<IndexError> = vec![
            IndexError::NotFound {
                path: "/tmp".into(),
            },
            IndexError::SerializationFailed {
                message: "x".into(),
            },
            IndexError::DeserializationFailed {
                message: "x".into(),
            },
            IndexError::WriteFailed {
                message: "x".into(),
            },
            IndexError::ReadFailed {
                message: "x".into(),
            },
            IndexError::Empty,
            IndexError::VersionMismatch {
                expected: 2,
                found: 1,
            },
            IndexError::NotInRepo,
        ];

        for err in &errors {
            assert!(!err.hint().is_empty());
            assert!(err.code().starts_with("E3"));
        }
    }

    #[test]
    fn test_version_mismatch_display() {
        let err = IndexError::VersionMismatch {
            expected: 2,
            found: 1,
        };
        assert_eq!(
            err.to_string(),
            "Index version mismatch: expected 2, found 1"
        );
    }
}
