use crate::errors::SearchError;
use crate::github::PrData;

/// Filters applied to search results.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Filter by PR author (case-insensitive partial match)
    pub author: Option<String>,
    /// Filter by PR label (case-insensitive)
    pub label: Option<String>,
    /// Filter by PR state (open/closed/merged)
    pub state: Option<String>,
    /// Filter PRs created after this date (YYYY-MM-DD)
    pub after: Option<String>,
    /// Filter PRs created before this date (YYYY-MM-DD)
    pub before: Option<String>,
}

impl SearchFilter {
    /// Check if a PR matches all active filters.
    pub fn matches(&self, pr: &PrData) -> bool {
        if let Some(ref author) = self.author {
            if !pr.author.to_lowercase().contains(&author.to_lowercase()) {
                return false;
            }
        }

        if let Some(ref label) = self.label {
            let label_lower = label.to_lowercase();
            if !pr
                .labels
                .iter()
                .any(|l| l.to_lowercase().contains(&label_lower))
            {
                return false;
            }
        }

        if let Some(ref state) = self.state {
            if pr.state.as_str() != state.to_lowercase() {
                return false;
            }
        }

        if let Some(ref after) = self.after {
            if let Ok(date) = parse_date(after) {
                if pr.created_at < date {
                    return false;
                }
            }
        }

        if let Some(ref before) = self.before {
            if let Ok(date) = parse_date(before) {
                if pr.created_at > date {
                    return false;
                }
            }
        }

        true
    }

    /// Validate all filter values. Returns an error if any filter is invalid.
    pub fn validate(&self) -> Result<(), SearchError> {
        if let Some(ref after) = self.after {
            parse_date(after).map_err(|_| SearchError::InvalidDateFilter {
                message: format!("Invalid 'after' date: {after}"),
            })?;
        }
        if let Some(ref before) = self.before {
            parse_date(before).map_err(|_| SearchError::InvalidDateFilter {
                message: format!("Invalid 'before' date: {before}"),
            })?;
        }
        if let Some(ref state) = self.state {
            match state.to_lowercase().as_str() {
                "open" | "closed" | "merged" => {}
                _ => {
                    return Err(SearchError::InvalidDateFilter {
                        message: format!(
                            "Invalid state filter: '{state}'. Must be open, closed, or merged"
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Parse a YYYY-MM-DD date string to a DateTime<Utc>.
fn parse_date(s: &str) -> Result<chrono::DateTime<chrono::Utc>, chrono::ParseError> {
    let naive = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")?;
    let dt = naive
        .and_hms_opt(0, 0, 0)
        .expect("midnight should always be valid");
    Ok(chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{PrData, PrState};
    use chrono::Utc;

    fn sample_pr() -> PrData {
        PrData {
            number: 1,
            title: "Test PR".into(),
            body: "".into(),
            author: "Alice".into(),
            state: PrState::Open,
            labels: vec!["Bug".into(), "Critical".into()],
            created_at: chrono::DateTime::parse_from_rfc3339("2025-06-15T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            updated_at: Utc::now(),
            merged_at: None,
            closed_at: None,
            html_url: "url".into(),
            review_comments: vec![],
            diff: None,
            repo: "o/r".into(),
        }
    }

    #[test]
    fn test_no_filters_matches_everything() {
        let filter = SearchFilter::default();
        assert!(filter.matches(&sample_pr()));
    }

    #[test]
    fn test_author_filter_case_insensitive() {
        let filter = SearchFilter {
            author: Some("alice".into()),
            ..Default::default()
        };
        assert!(filter.matches(&sample_pr()));
    }

    #[test]
    fn test_author_filter_partial_match() {
        let filter = SearchFilter {
            author: Some("ali".into()),
            ..Default::default()
        };
        assert!(filter.matches(&sample_pr()));
    }

    #[test]
    fn test_author_filter_no_match() {
        let filter = SearchFilter {
            author: Some("bob".into()),
            ..Default::default()
        };
        assert!(!filter.matches(&sample_pr()));
    }

    #[test]
    fn test_label_filter() {
        let filter = SearchFilter {
            label: Some("bug".into()),
            ..Default::default()
        };
        assert!(filter.matches(&sample_pr()));
    }

    #[test]
    fn test_label_filter_partial() {
        let filter = SearchFilter {
            label: Some("crit".into()),
            ..Default::default()
        };
        assert!(filter.matches(&sample_pr()));
    }

    #[test]
    fn test_label_filter_no_match() {
        let filter = SearchFilter {
            label: Some("feature".into()),
            ..Default::default()
        };
        assert!(!filter.matches(&sample_pr()));
    }

    #[test]
    fn test_state_filter() {
        let filter = SearchFilter {
            state: Some("open".into()),
            ..Default::default()
        };
        assert!(filter.matches(&sample_pr()));

        let filter = SearchFilter {
            state: Some("merged".into()),
            ..Default::default()
        };
        assert!(!filter.matches(&sample_pr()));
    }

    #[test]
    fn test_after_filter() {
        let filter = SearchFilter {
            after: Some("2025-01-01".into()),
            ..Default::default()
        };
        assert!(filter.matches(&sample_pr()));

        let filter = SearchFilter {
            after: Some("2025-12-01".into()),
            ..Default::default()
        };
        assert!(!filter.matches(&sample_pr()));
    }

    #[test]
    fn test_before_filter() {
        let filter = SearchFilter {
            before: Some("2025-12-31".into()),
            ..Default::default()
        };
        assert!(filter.matches(&sample_pr()));

        let filter = SearchFilter {
            before: Some("2025-01-01".into()),
            ..Default::default()
        };
        assert!(!filter.matches(&sample_pr()));
    }

    #[test]
    fn test_combined_filters() {
        let filter = SearchFilter {
            author: Some("alice".into()),
            label: Some("bug".into()),
            state: Some("open".into()),
            after: Some("2025-01-01".into()),
            before: Some("2025-12-31".into()),
        };
        assert!(filter.matches(&sample_pr()));
    }

    #[test]
    fn test_combined_filters_one_fails() {
        let filter = SearchFilter {
            author: Some("alice".into()),
            state: Some("merged".into()), // PR is open, not merged
            ..Default::default()
        };
        assert!(!filter.matches(&sample_pr()));
    }

    #[test]
    fn test_validate_valid_dates() {
        let filter = SearchFilter {
            after: Some("2025-01-01".into()),
            before: Some("2025-12-31".into()),
            ..Default::default()
        };
        assert!(filter.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_date() {
        let filter = SearchFilter {
            after: Some("not-a-date".into()),
            ..Default::default()
        };
        assert!(filter.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_state() {
        let filter = SearchFilter {
            state: Some("pending".into()),
            ..Default::default()
        };
        assert!(filter.validate().is_err());
    }

    #[test]
    fn test_validate_valid_states() {
        for state in &["open", "closed", "merged", "Open", "MERGED"] {
            let filter = SearchFilter {
                state: Some(state.to_string()),
                ..Default::default()
            };
            assert!(filter.validate().is_ok(), "State '{state}' should be valid");
        }
    }
}
