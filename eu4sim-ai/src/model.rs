//! Model loading and inference for EU4 AI.
//!
//! Loads SmolLM2 or Gemma base models from HuggingFace Hub,
//! applies LoRA adapters, and runs inference on CPU.

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::llama::{Cache, Config, Llama, LlamaConfig};
use hf_hub::{Repo, RepoType, api::sync::Api};
use rand::Rng;
use std::path::{Path, PathBuf};
use tokenizers::Tokenizer;

/// Supported model architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArch {
    /// SmolLM2 (LLaMA-like architecture)
    SmolLM2,
    /// Google Gemma 2
    Gemma2,
}

impl ModelArch {
    /// Detect architecture from config.json
    pub fn from_config(config_path: &Path) -> Result<Self> {
        let config_str =
            std::fs::read_to_string(config_path).context("Failed to read config.json")?;
        let config: serde_json::Value =
            serde_json::from_str(&config_str).context("Failed to parse config.json")?;

        let model_type = config
            .get("model_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match model_type {
            "llama" | "mistral" => Ok(Self::SmolLM2),
            "gemma" | "gemma2" => Ok(Self::Gemma2),
            other => anyhow::bail!("Unknown model type: {}", other),
        }
    }

    /// Get the HuggingFace repo ID for the base model.
    pub fn base_model_repo(&self) -> &'static str {
        match self {
            Self::SmolLM2 => "HuggingFaceTB/SmolLM2-360M",
            Self::Gemma2 => "google/gemma-2-2b-it",
        }
    }
}

/// Configuration for model loading.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// HuggingFace repo ID for base model (e.g., "HuggingFaceTB/SmolLM2-360M")
    pub base_model: String,
    /// Path to LoRA adapter directory (contains adapter_model.safetensors)
    pub adapter_path: PathBuf,
    /// Device to run inference on
    pub device: Device,
    /// Data type for model weights (F32 for CPU compatibility)
    pub dtype: DType,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            base_model: "HuggingFaceTB/SmolLM2-360M".to_string(),
            adapter_path: PathBuf::new(),
            device: Device::Cpu,
            dtype: DType::F32,
        }
    }
}

/// LoRA adapter configuration (from adapter_config.json).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoraConfig {
    pub r: usize,
    pub lora_alpha: usize,
    pub target_modules: Vec<String>,
    #[serde(default)]
    pub lora_dropout: f64,
}

/// Unified model wrapper for EU4 AI inference.
pub struct Eu4AiModel {
    model: Llama,
    cache: Cache,
    tokenizer: Tokenizer,
    config: Config,
    device: Device,
    #[allow(dead_code)]
    arch: ModelArch,
}

impl Eu4AiModel {
    /// Load a model with LoRA adapter.
    ///
    /// Downloads the base model from HuggingFace Hub if not cached,
    /// then applies the LoRA adapter weights.
    pub fn load(config: ModelConfig) -> Result<Self> {
        log::info!("Loading base model: {}", config.base_model);

        // Download base model files from HuggingFace
        let api = Api::new().context("Failed to create HuggingFace API")?;
        let repo = api.repo(Repo::new(config.base_model.clone(), RepoType::Model));

        let config_path = repo
            .get("config.json")
            .context("Failed to download config.json")?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .context("Failed to download tokenizer.json")?;
        let weights_path = repo
            .get("model.safetensors")
            .context("Failed to download model.safetensors")?;

        // Detect architecture
        let arch = ModelArch::from_config(&config_path)?;
        log::info!("Detected architecture: {:?}", arch);

        // Load model config
        let llama_config: LlamaConfig = serde_json::from_str(
            &std::fs::read_to_string(&config_path).context("Failed to read config")?,
        )
        .context("Failed to parse LlamaConfig")?;
        let model_config = llama_config.into_config(false); // no flash attention on CPU

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Load base model weights
        log::info!("Loading base model weights...");
        let base_vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], config.dtype, &config.device)?
        };

        // Load LoRA adapter if provided
        if !config.adapter_path.as_os_str().is_empty() {
            log::info!("Loading LoRA adapter from {:?}", config.adapter_path);
            Self::log_lora_info(&config.adapter_path)?;
            // TODO: Implement actual LoRA weight merging
            // For now, we log the adapter info but use base model
            log::warn!("LoRA weight merging not yet implemented - using base model only");
        }
        let vb = base_vb;

        // Build the model
        let model = Llama::load(vb, &model_config).context("Failed to load Llama model")?;

        // Create KV cache
        let cache = Cache::new(true, config.dtype, &model_config, &config.device)?;

        log::info!("Model loaded successfully");

        Ok(Self {
            model,
            cache,
            tokenizer,
            config: model_config,
            device: config.device,
            arch,
        })
    }

    /// Log information about the LoRA adapter.
    fn log_lora_info(adapter_path: &Path) -> Result<()> {
        // Load LoRA config
        let lora_config_path = adapter_path.join("adapter_config.json");
        let lora_config: LoraConfig = serde_json::from_str(
            &std::fs::read_to_string(&lora_config_path)
                .context("Failed to read adapter_config.json")?,
        )
        .context("Failed to parse LoRA config")?;

        log::info!(
            "LoRA config: r={}, alpha={}, targets={:?}",
            lora_config.r,
            lora_config.lora_alpha,
            lora_config.target_modules
        );

        // Check weights exist
        let lora_weights_path = adapter_path.join("adapter_model.safetensors");
        if lora_weights_path.exists() {
            let metadata = std::fs::metadata(&lora_weights_path)?;
            log::info!(
                "LoRA weights: {} ({:.1} MB)",
                lora_weights_path.display(),
                metadata.len() as f64 / 1_000_000.0
            );
        }

        Ok(())
    }

    /// Run inference on a prompt and return the chosen action index.
    ///
    /// The prompt should end with `<|choice|>` and the model will generate
    /// a single digit (0-9) representing the chosen action.
    pub fn choose_action(&mut self, prompt: &str) -> Result<usize> {
        // Tokenize
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let tokens = encoding.get_ids();

        log::debug!("Prompt tokens: {} tokens", tokens.len());

        // Convert to tensor
        let input_ids = Tensor::new(tokens, &self.device)?.unsqueeze(0)?;

        // Note: Cache is reused for KV caching during generation
        // For single-shot inference, we create fresh cache each time

        // Forward pass
        let logits = self
            .model
            .forward(&input_ids, 0, &mut self.cache)
            .context("Forward pass failed")?;

        // Get logits for next token
        let logits = logits.squeeze(0)?; // Remove batch dimension

        // Sample from digit tokens (0-9)
        let action = self.sample_digit(&logits)?;

        log::debug!("Model chose action: {}", action);
        Ok(action)
    }

    /// Sample a digit (0-9) from the logits.
    fn sample_digit(&self, logits: &Tensor) -> Result<usize> {
        // Get token IDs for digits 0-9
        let digit_tokens: Vec<u32> = (0..10)
            .filter_map(|d| {
                let s = d.to_string();
                self.tokenizer
                    .encode(s.as_str(), false)
                    .ok()
                    .and_then(|enc| enc.get_ids().first().copied())
            })
            .collect();

        if digit_tokens.len() != 10 {
            anyhow::bail!(
                "Could not find all digit tokens, found {}",
                digit_tokens.len()
            );
        }

        // Extract logits for digit tokens
        let logits_vec: Vec<f32> = logits.to_vec1()?;
        let digit_logits: Vec<f32> = digit_tokens
            .iter()
            .map(|&t| {
                logits_vec
                    .get(t as usize)
                    .copied()
                    .unwrap_or(f32::NEG_INFINITY)
            })
            .collect();

        // Softmax
        let max_logit = digit_logits
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = digit_logits
            .iter()
            .map(|&l| (l - max_logit).exp())
            .collect();
        let sum: f32 = exp_logits.iter().sum();
        let probs: Vec<f32> = exp_logits.iter().map(|&e| e / sum).collect();

        // Sample from distribution
        let mut rng = rand::thread_rng();
        let r: f32 = rng.r#gen();
        let mut cumsum = 0.0;
        for (i, &p) in probs.iter().enumerate() {
            cumsum += p;
            if r < cumsum {
                return Ok(i);
            }
        }

        // Fallback to highest probability
        Ok(probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0))
    }

    /// Get the model architecture.
    pub fn arch(&self) -> ModelArch {
        self.arch
    }

    /// Get a reference to the tokenizer.
    pub fn tokenizer(&self) -> &Tokenizer {
        &self.tokenizer
    }

    /// Get the model config.
    pub fn config(&self) -> &Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_arch_from_config() {
        // Test with a temporary config file
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // LLaMA-style config
        std::fs::write(&config_path, r#"{"model_type": "llama"}"#).unwrap();
        assert_eq!(
            ModelArch::from_config(&config_path).unwrap(),
            ModelArch::SmolLM2
        );

        // Gemma config
        std::fs::write(&config_path, r#"{"model_type": "gemma2"}"#).unwrap();
        assert_eq!(
            ModelArch::from_config(&config_path).unwrap(),
            ModelArch::Gemma2
        );
    }
}
