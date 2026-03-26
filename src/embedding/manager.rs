use std::path::PathBuf;

use anyhow::{Context, Result};
use ndarray::Array2;
use ort::value::Tensor;

use crate::errors::EmbeddingError;

use super::{l2_normalize, EMBEDDING_DIM, MAX_TOKENS, MODEL_REPO};

/// Manages the ONNX embedding model lifecycle: download, load, and inference.
pub struct ModelManager {
    model_dir: PathBuf,
    session: Option<ort::session::Session>,
    tokenizer: Option<tokenizers::Tokenizer>,
}

impl ModelManager {
    /// Create a new ModelManager. The model will be stored in the given directory.
    pub fn new(model_dir: PathBuf) -> Self {
        Self {
            model_dir,
            session: None,
            tokenizer: None,
        }
    }

    /// Get the default model directory (platform-appropriate).
    pub fn default_model_dir() -> PathBuf {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "pr-search", "pr-search") {
            proj_dirs.data_dir().to_path_buf()
        } else {
            PathBuf::from(".pr-search-models")
        }
    }

    /// Path to the ONNX model file.
    pub fn model_path(&self) -> PathBuf {
        self.model_dir.join("model.onnx")
    }

    /// Path to the tokenizer file.
    pub fn tokenizer_path(&self) -> PathBuf {
        self.model_dir.join("tokenizer.json")
    }

    /// Check if the model files exist.
    pub fn is_initialized(&self) -> bool {
        self.model_path().exists() && self.tokenizer_path().exists()
    }

    /// Download model files from HuggingFace.
    pub fn download(&self, force: bool) -> Result<()> {
        if self.is_initialized() && !force {
            tracing::info!("Model already exists, skipping download (use --force to re-download)");
            return Ok(());
        }

        std::fs::create_dir_all(&self.model_dir).context("Failed to create model directory")?;

        let model_url = format!(
            "https://huggingface.co/{}/resolve/main/onnx/model.onnx",
            MODEL_REPO
        );
        let tokenizer_url = format!(
            "https://huggingface.co/{}/resolve/main/tokenizer.json",
            MODEL_REPO
        );

        tracing::info!("Downloading ONNX model from HuggingFace...");
        self.download_file(&model_url, &self.model_path())?;

        tracing::info!("Downloading tokenizer...");
        self.download_file(&tokenizer_url, &self.tokenizer_path())?;

        tracing::info!("Model initialization complete");
        Ok(())
    }

    /// Download a single file from a URL.
    fn download_file(&self, url: &str, dest: &std::path::Path) -> Result<()> {
        let response = reqwest::blocking::get(url).map_err(|e| EmbeddingError::DownloadFailed {
            message: e.to_string(),
        })?;

        if !response.status().is_success() {
            return Err(EmbeddingError::DownloadFailed {
                message: format!("HTTP {}", response.status()),
            }
            .into());
        }

        let bytes = response
            .bytes()
            .map_err(|e| EmbeddingError::DownloadFailed {
                message: e.to_string(),
            })?;

        // Write to temp file then rename for atomicity (avoids corrupt files on interrupted downloads)
        let tmp_dest = dest.with_extension("tmp");
        std::fs::write(&tmp_dest, &bytes).context("Failed to write model file to disk")?;
        std::fs::rename(&tmp_dest, dest).context("Failed to finalize model file")?;

        tracing::info!(
            path = %dest.display(),
            size_mb = bytes.len() / (1024 * 1024),
            "Downloaded file"
        );

        Ok(())
    }

    /// Load the model and tokenizer into memory.
    pub fn load(&mut self) -> Result<()> {
        if !self.is_initialized() {
            return Err(EmbeddingError::ModelNotFound {
                path: self.model_dir.display().to_string(),
            }
            .into());
        }

        // Load ONNX model
        let session = ort::session::Session::builder()
            .and_then(|mut builder| builder.commit_from_file(self.model_path()))
            .map_err(|e| EmbeddingError::ModelLoadFailed {
                message: e.to_string(),
            })?;

        self.session = Some(session);

        // Load tokenizer
        let tokenizer = tokenizers::Tokenizer::from_file(self.tokenizer_path()).map_err(|e| {
            EmbeddingError::TokenizerLoadFailed {
                message: e.to_string(),
            }
        })?;

        self.tokenizer = Some(tokenizer);

        tracing::info!("Model and tokenizer loaded successfully");
        Ok(())
    }

    /// Generate an embedding for the given text.
    /// Returns a normalized 384-dimensional vector.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let session = self
            .session
            .as_mut()
            .ok_or(EmbeddingError::NotInitialized)?;
        let tokenizer = self
            .tokenizer
            .as_ref()
            .ok_or(EmbeddingError::NotInitialized)?;

        // Tokenize
        let encoding =
            tokenizer
                .encode(text, true)
                .map_err(|e| EmbeddingError::InferenceFailed {
                    message: format!("Tokenization failed: {e}"),
                })?;

        // Truncate to MAX_TOKENS
        let ids = encoding.get_ids();
        let attention = encoding.get_attention_mask();
        let type_ids = encoding.get_type_ids();

        let len = ids.len().min(MAX_TOKENS);

        let input_ids: Vec<i64> = ids[..len].iter().map(|&x| x as i64).collect();
        let attention_mask: Vec<i64> = attention[..len].iter().map(|&x| x as i64).collect();
        let token_type_ids: Vec<i64> = type_ids[..len].iter().map(|&x| x as i64).collect();

        // Create input tensors
        let input_ids_array = Array2::from_shape_vec((1, len), input_ids)
            .context("Failed to create input_ids tensor")?;
        let attention_mask_array = Array2::from_shape_vec((1, len), attention_mask)
            .context("Failed to create attention_mask tensor")?;
        let token_type_ids_array = Array2::from_shape_vec((1, len), token_type_ids)
            .context("Failed to create token_type_ids tensor")?;

        // Convert to ort tensors
        let input_ids_tensor =
            Tensor::from_array(input_ids_array).map_err(|e| EmbeddingError::InferenceFailed {
                message: format!("Failed to create input tensor: {e}"),
            })?;
        let attention_mask_tensor = Tensor::from_array(attention_mask_array).map_err(|e| {
            EmbeddingError::InferenceFailed {
                message: format!("Failed to create attention mask tensor: {e}"),
            }
        })?;
        let token_type_ids_tensor = Tensor::from_array(token_type_ids_array).map_err(|e| {
            EmbeddingError::InferenceFailed {
                message: format!("Failed to create token type ids tensor: {e}"),
            }
        })?;

        // Run inference
        let outputs = session
            .run(ort::inputs! {
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => token_type_ids_tensor,
            })
            .map_err(|e| EmbeddingError::InferenceFailed {
                message: e.to_string(),
            })?;

        // Extract CLS token embedding (first token)
        let (_shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(|e| {
            EmbeddingError::InferenceFailed {
                message: format!("Failed to extract output tensor: {e}"),
            }
        })?;

        // Shape: [1, seq_len, 384] — take first token (CLS), which starts at index 0
        let mut embedding: Vec<f32> = data[..EMBEDDING_DIM].to_vec();

        // L2-normalize
        l2_normalize(&mut embedding);

        Ok(embedding)
    }

    /// Generate embeddings for multiple texts.
    /// Processes one at a time (batching can be added later for performance).
    pub fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|text| self.embed(text)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_dir() {
        let dir = ModelManager::default_model_dir();
        // Should return some valid path
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_model_paths() {
        let mgr = ModelManager::new(PathBuf::from("/tmp/test-models"));
        assert_eq!(
            mgr.model_path(),
            PathBuf::from("/tmp/test-models/model.onnx")
        );
        assert_eq!(
            mgr.tokenizer_path(),
            PathBuf::from("/tmp/test-models/tokenizer.json")
        );
    }

    #[test]
    fn test_not_initialized_by_default() {
        let mgr = ModelManager::new(PathBuf::from("/nonexistent/path"));
        assert!(!mgr.is_initialized());
    }

    #[test]
    fn test_embed_without_loading_returns_error() {
        let mut mgr = ModelManager::new(PathBuf::from("/tmp/test"));
        let result = mgr.embed("test text");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not initialized"));
    }

    #[test]
    fn test_load_nonexistent_model_returns_error() {
        let mut mgr = ModelManager::new(PathBuf::from("/nonexistent/model/path"));
        let result = mgr.load();
        assert!(result.is_err());
    }
}
