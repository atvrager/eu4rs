//! Model loading and inference for EU4 AI.
//!
//! Supports multiple model architectures with a unified interface.
//! Currently a stub - actual implementation requires testing with real model files.

use anyhow::{Context, Result};
use candle_core::{DType, Device};
use std::path::Path;
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
}

/// Configuration for model loading.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Path to the base model directory (contains config.json, model.safetensors, tokenizer.json)
    pub model_path: std::path::PathBuf,
    /// Optional path to LoRA adapter directory
    pub lora_path: Option<std::path::PathBuf>,
    /// Device to run inference on (CPU by default for accessibility)
    pub device: Device,
    /// Data type for model weights
    pub dtype: DType,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_path: std::path::PathBuf::new(),
            lora_path: None,
            device: Device::Cpu,
            dtype: DType::F32,
        }
    }
}

/// Unified model wrapper for EU4 AI inference.
///
/// Wraps different model architectures (SmolLM2, Gemma) with a common interface.
/// Runs on CPU by default for maximum accessibility.
pub struct Eu4AiModel {
    tokenizer: Tokenizer,
    #[allow(dead_code)]
    device: Device,
    #[allow(dead_code)]
    arch: ModelArch,
    // Model weights will be stored here once we finalize the candle integration
    // For now, we just validate the config and tokenizer can load
}

impl Eu4AiModel {
    /// Load a model from disk.
    ///
    /// The model directory should contain:
    /// - `config.json` - Model configuration
    /// - `model.safetensors` - Model weights
    /// - `tokenizer.json` - Tokenizer configuration
    ///
    /// Optionally, a LoRA adapter directory can be specified for personality variants.
    pub fn load(config: ModelConfig) -> Result<Self> {
        let arch = ModelArch::from_config(&config.model_path.join("config.json"))?;
        log::info!("Detected model architecture: {:?}", arch);

        // Load tokenizer
        let tokenizer_path = config.model_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Verify model weights exist
        let model_path = config.model_path.join("model.safetensors");
        if !model_path.exists() {
            anyhow::bail!("Model weights not found: {:?}", model_path);
        }

        // TODO: Actually load model weights with candle
        // This requires careful handling of the model-specific configs
        // For now, we just validate the files exist
        log::info!("Model files validated. Full inference not yet implemented.");

        if let Some(lora_path) = &config.lora_path {
            log::info!("LoRA adapter path: {:?}", lora_path);
            // TODO: Load and apply LoRA weights
        }

        Ok(Self {
            tokenizer,
            device: config.device,
            arch,
        })
    }

    /// Run inference on a prompt and return the chosen action index.
    ///
    /// The prompt should end with `<|choice|>` and the model will generate
    /// a single digit (0-9) representing the chosen action.
    ///
    /// # Returns
    /// The action index (0-9) chosen by the model.
    pub fn choose_action(&mut self, prompt: &str) -> Result<usize> {
        // Tokenize to validate the prompt
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let _tokens = encoding.get_ids();

        // TODO: Run actual inference
        // For now, return a placeholder
        log::warn!("Full inference not implemented - returning action 0");
        Ok(0)
    }

    /// Get the model architecture.
    pub fn arch(&self) -> ModelArch {
        self.arch
    }

    /// Get a reference to the tokenizer.
    pub fn tokenizer(&self) -> &Tokenizer {
        &self.tokenizer
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
