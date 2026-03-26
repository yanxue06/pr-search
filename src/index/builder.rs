use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::embedding::ModelManager;
use crate::github::PrData;

use super::{IndexEntry, SemanticIndex};

/// Builds a semantic index from PR data.
pub struct IndexBuilder<'a> {
    model: &'a mut ModelManager,
}

impl<'a> IndexBuilder<'a> {
    /// Create a new builder with a loaded ModelManager.
    pub fn new(model: &'a mut ModelManager) -> Self {
        Self { model }
    }

    /// Build a fresh index from a list of PRs.
    pub fn build(&mut self, repo: &str, prs: &[PrData], with_diffs: bool) -> Result<SemanticIndex> {
        let mut index = SemanticIndex::new(repo, with_diffs);
        self.add_prs_to_index(&mut index, prs)?;
        Ok(index)
    }

    /// Incrementally update an existing index with new PRs.
    /// Only PRs not already in the index will be embedded and added.
    pub fn update(&mut self, existing: &mut SemanticIndex, new_prs: &[PrData]) -> Result<usize> {
        let existing_numbers = existing.indexed_pr_numbers();
        let prs_to_add: Vec<&PrData> = new_prs
            .iter()
            .filter(|pr| !existing_numbers.contains(&pr.number))
            .collect();

        if prs_to_add.is_empty() {
            tracing::info!("No new PRs to index");
            return Ok(0);
        }

        let count = prs_to_add.len();
        tracing::info!(new_prs = count, "Adding new PRs to index");

        let pb = ProgressBar::new(count as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} embeddings ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );

        for pr in prs_to_add {
            let text = pr.to_embedding_text();
            let embedding = self.model.embed(&text)?;

            existing.add_entry(IndexEntry {
                pr: pr.clone(),
                embedding,
            });

            pb.inc(1);
        }

        pb.finish_with_message("Done embedding PRs");
        existing.deduplicate();

        Ok(count)
    }

    /// Add PRs to an index (internal helper).
    fn add_prs_to_index(&mut self, index: &mut SemanticIndex, prs: &[PrData]) -> Result<()> {
        let pb = ProgressBar::new(prs.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} embeddings ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );

        for pr in prs {
            let text = pr.to_embedding_text();
            let embedding = self.model.embed(&text)?;

            index.add_entry(IndexEntry {
                pr: pr.clone(),
                embedding,
            });

            pb.inc(1);
        }

        pb.finish_with_message("Done embedding PRs");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_requires_model() {
        // Just verify the builder compiles and can be constructed
        // Actual embedding tests require the ONNX model to be downloaded
        let mut model = ModelManager::new(std::path::PathBuf::from("/tmp/nonexistent"));
        let _builder = IndexBuilder::new(&mut model);
    }
}
