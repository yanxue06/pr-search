use super::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Empty search query")]
    EmptyQuery,

    #[error("No results found for query: {query}")]
    NoResults { query: String },

    #[error("Invalid date filter: {message}")]
    InvalidDateFilter { message: String },
}

impl DomainError for SearchError {
    fn hint(&self) -> &str {
        match self {
            Self::EmptyQuery => "Provide a search query, e.g.: pr-search search \"fix auth bug\"",
            Self::NoResults { .. } => "Try different search terms, or broaden your filters",
            Self::InvalidDateFilter { .. } => "Use ISO 8601 format: YYYY-MM-DD (e.g., 2026-01-15)",
        }
    }

    fn code(&self) -> &str {
        match self {
            Self::EmptyQuery => "E4001",
            Self::NoResults { .. } => "E4002",
            Self::InvalidDateFilter { .. } => "E4003",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_errors_have_hints_and_codes() {
        let errors: Vec<SearchError> = vec![
            SearchError::EmptyQuery,
            SearchError::NoResults {
                query: "test".into(),
            },
            SearchError::InvalidDateFilter {
                message: "bad date".into(),
            },
        ];

        for err in &errors {
            assert!(!err.hint().is_empty());
            assert!(err.code().starts_with("E4"));
        }
    }
}
