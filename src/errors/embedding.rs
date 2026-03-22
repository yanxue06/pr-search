use super::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Model not found at {path}")]
    ModelNotFound { path: String },

    #[error("Failed to download model: {message}")]
    DownloadFailed { message: String },

    #[error("Failed to load ONNX model: {message}")]
    ModelLoadFailed { message: String },

    #[error("Failed to load tokenizer: {message}")]
    TokenizerLoadFailed { message: String },

    #[error("Inference failed: {message}")]
    InferenceFailed { message: String },

    #[error("Model not initialized — call `init` first")]
    NotInitialized,
}

impl DomainError for EmbeddingError {
    fn hint(&self) -> &str {
        match self {
            Self::ModelNotFound { .. } => "Run: pr-search init",
            Self::DownloadFailed { .. } => "Check your network connection and try again",
            Self::ModelLoadFailed { .. } => "The model file may be corrupted. Run: pr-search init --force",
            Self::TokenizerLoadFailed { .. } => "The tokenizer file may be corrupted. Run: pr-search init --force",
            Self::InferenceFailed { .. } => "This may be a bug. Please report it at https://github.com/yanxue06/pr-search/issues",
            Self::NotInitialized => "Run: pr-search init",
        }
    }

    fn code(&self) -> &str {
        match self {
            Self::ModelNotFound { .. } => "E1001",
            Self::DownloadFailed { .. } => "E1002",
            Self::ModelLoadFailed { .. } => "E1003",
            Self::TokenizerLoadFailed { .. } => "E1004",
            Self::InferenceFailed { .. } => "E1005",
            Self::NotInitialized => "E1006",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_errors_have_hints_and_codes() {
        let errors: Vec<EmbeddingError> = vec![
            EmbeddingError::ModelNotFound {
                path: "/tmp/model".into(),
            },
            EmbeddingError::DownloadFailed {
                message: "timeout".into(),
            },
            EmbeddingError::ModelLoadFailed {
                message: "corrupt".into(),
            },
            EmbeddingError::TokenizerLoadFailed {
                message: "missing".into(),
            },
            EmbeddingError::InferenceFailed {
                message: "oom".into(),
            },
            EmbeddingError::NotInitialized,
        ];

        for err in &errors {
            assert!(!err.hint().is_empty());
            assert!(err.code().starts_with("E1"));
        }
    }
}
