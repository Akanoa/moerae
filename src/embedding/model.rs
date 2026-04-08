use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use sha2::{Digest, Sha256};

use crate::error::InitError;

static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

fn get_backend() -> &'static LlamaBackend {
    BACKEND.get_or_init(|| {
        LlamaBackend::init().expect("failed to initialize llama backend")
    })
}

pub struct EmbeddingModel {
    model: LlamaModel,
    dimensions: usize,
    max_tokens: usize,
    model_id_str: String,
}

impl EmbeddingModel {
    pub fn load(path: &Path) -> Result<Self, InitError> {
        if !path.exists() {
            download_model(path)?;
        }

        let backend = get_backend();

        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(backend, path, &model_params)
            .map_err(|e| InitError::ModelLoadFailed(e.to_string()))?;

        let dimensions = model.n_embd() as usize;
        let max_tokens = model.n_ctx_train() as usize;
        let model_id_str = compute_model_id(path, dimensions)?;

        Ok(Self {
            model,
            dimensions,
            max_tokens,
            model_id_str,
        })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let backend = get_backend();

        let tokens = self
            .model
            .str_to_token(text, AddBos::Always)
            .map_err(|e| e.to_string())?;

        let n_tokens = tokens.len();
        let ctx_size = n_tokens.max(32); // minimum context size

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZero::new(ctx_size as u32))
            .with_embeddings(true);

        let mut ctx = self
            .model
            .new_context(backend, ctx_params)
            .map_err(|e| e.to_string())?;

        let mut batch = LlamaBatch::new(ctx_size, 1);
        batch
            .add_sequence(&tokens, 0, false)
            .map_err(|e| e.to_string())?;

        ctx.clear_kv_cache();
        ctx.decode(&mut batch).map_err(|e| e.to_string())?;

        let embedding = ctx.embeddings_seq_ith(0).map_err(|e| e.to_string())?;

        Ok(normalize(embedding))
    }

    pub fn token_count(&self, text: &str) -> Result<usize, String> {
        let tokens = self
            .model
            .str_to_token(text, AddBos::Always)
            .map_err(|e| e.to_string())?;
        Ok(tokens.len())
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    pub fn model_id(&self) -> &str {
        &self.model_id_str
    }

    pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect()
    }
}

pub fn default_model_path() -> PathBuf {
    let base = if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".moerae")
    } else {
        PathBuf::from(".moerae")
    };
    base.join("models")
        .join("embeddinggemma-300m-qat-q8_0.gguf")
}

const MODEL_URL: &str = "https://huggingface.co/ggml-org/embeddinggemma-300m-qat-q8_0-GGUF/resolve/main/embeddinggemma-300m-qat-Q8_0.gguf";

fn download_model(path: &Path) -> Result<(), InitError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    eprintln!("Model not found, downloading embeddinggemma-300m...");

    let response = ureq::get(MODEL_URL)
        .call()
        .map_err(|e| InitError::ModelDownloadFailed(e.to_string()))?;

    let total_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    let tmp_path = path.with_extension("gguf.tmp");
    let mut file = fs::File::create(&tmp_path)?;
    let mut reader = response.into_body().into_reader();
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 64 * 1024];
    let mut last_pct = 0;

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| InitError::ModelDownloadFailed(e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;

        if let Some(total) = total_size {
            let pct = (downloaded * 100 / total) as u8;
            if pct != last_pct {
                last_pct = pct;
                eprint!("\rDownloading model... {pct}% ({downloaded}/{total} bytes)");
            }
        }
    }
    drop(file);

    fs::rename(&tmp_path, path)?;
    eprintln!("\rModel downloaded to {}", path.display());

    Ok(())
}

fn compute_model_id(path: &Path, dimensions: usize) -> Result<String, InitError> {
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len();

    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; 4096.min(file_size as usize)];
    file.read_exact(&mut buf)?;

    let mut hasher = Sha256::new();
    hasher.update(&buf);
    let hash = hasher.finalize();
    let hash_hex = &format!("{:x}", hash)[..16];

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    Ok(format!("{name}_{dimensions}_{file_size}_{hash_hex}"))
}

fn normalize(input: &[f32]) -> Vec<f32> {
    let magnitude = input
        .iter()
        .fold(0.0f32, |acc, &val| val.mul_add(val, acc))
        .sqrt();
    if magnitude == 0.0 {
        return input.to_vec();
    }
    input.iter().map(|&val| val / magnitude).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_path_is_reasonable() {
        let path = default_model_path();
        assert!(path.to_str().unwrap().contains("embeddinggemma"));
        assert!(path.to_str().unwrap().ends_with(".gguf"));
    }

    #[test]
    fn embedding_bytes_roundtrip() {
        let embedding = vec![0.1, 0.2, 0.3, -0.5, 1.0];
        let bytes = EmbeddingModel::embedding_to_bytes(&embedding);
        let restored = EmbeddingModel::bytes_to_embedding(&bytes);
        assert_eq!(embedding.len(), restored.len());
        for (a, b) in embedding.iter().zip(restored.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn normalize_produces_unit_vector() {
        let input = vec![3.0, 4.0];
        let normed = normalize(&input);
        let magnitude: f32 = normed.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_zero_vector() {
        let input = vec![0.0, 0.0, 0.0];
        let normed = normalize(&input);
        assert_eq!(normed, input);
    }

    #[test]
    #[ignore] // Requires model file to be present
    fn load_and_embed() {
        let path = default_model_path();
        let model = EmbeddingModel::load(&path).unwrap();
        assert_eq!(model.dimensions(), 768);

        let embedding = model.embed("Hello, world!").unwrap();
        assert_eq!(embedding.len(), 768);

        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 1e-5);

        // Determinism
        let embedding2 = model.embed("Hello, world!").unwrap();
        assert_eq!(embedding, embedding2);
    }

    #[test]
    #[ignore] // Requires model file to be present
    fn token_count_works() {
        let path = default_model_path();
        let model = EmbeddingModel::load(&path).unwrap();
        let count = model.token_count("Hello, world!").unwrap();
        assert!(count > 0);
        assert!(count < 20);
    }

    #[test]
    #[ignore] // Requires model file to be present
    fn model_id_is_deterministic() {
        let path = default_model_path();
        let model = EmbeddingModel::load(&path).unwrap();
        let id1 = model.model_id().to_string();

        let model2 = EmbeddingModel::load(&path).unwrap();
        let id2 = model2.model_id().to_string();
        assert_eq!(id1, id2);
    }
}
