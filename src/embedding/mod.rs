mod manager;

pub use manager::ModelManager;

/// Embedding dimension for BGE-small-en-v1.5
pub const EMBEDDING_DIM: usize = 384;

/// Maximum number of tokens the model accepts
pub const MAX_TOKENS: usize = 512;

/// Model identifier on HuggingFace
pub const MODEL_REPO: &str = "BAAI/bge-small-en-v1.5";

/// L2-normalize a vector in place.
pub fn l2_normalize(vec: &mut [f32]) {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dim() {
        assert_eq!(EMBEDDING_DIM, 384);
    }

    #[test]
    fn test_max_tokens() {
        assert_eq!(MAX_TOKENS, 512);
    }

    #[test]
    fn test_l2_normalize_unit_vector() {
        let mut v = vec![1.0, 0.0, 0.0];
        l2_normalize(&mut v);
        assert!((v[0] - 1.0).abs() < 1e-6);
        assert!((v[1]).abs() < 1e-6);
        assert!((v[2]).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_general() {
        let mut v = vec![3.0, 4.0];
        l2_normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);

        // Check norm is 1
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let mut v = vec![0.0, 0.0, 0.0];
        l2_normalize(&mut v);
        // Should not panic, should remain zeros
        assert!(v.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_l2_normalize_384_dim() {
        let mut v: Vec<f32> = (0..EMBEDDING_DIM).map(|i| i as f32).collect();
        l2_normalize(&mut v);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }
}
