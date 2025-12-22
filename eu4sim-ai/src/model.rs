//! Model loading and inference for EU4 AI.
//!
//! Loads SmolLM2 or Gemma base models from HuggingFace Hub,
//! applies LoRA adapters, and runs inference on CPU.

use crate::device::{select_device, DevicePreference};
use anyhow::{Context, Result};
use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::llama::{Cache, Config, Llama, LlamaConfig};
use hf_hub::{api::sync::Api, Repo, RepoType};
use rand::Rng;
use safetensors::SafeTensors;
use std::collections::HashMap;
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
    /// Device preference (auto-selects best available device)
    pub device_pref: DevicePreference,
    /// Data type for model weights (F32 for CPU compatibility)
    pub dtype: DType,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            base_model: "HuggingFaceTB/SmolLM2-360M".to_string(),
            adapter_path: PathBuf::new(),
            device_pref: DevicePreference::default(),
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
    #[allow(dead_code)]
    cache: Cache,
    tokenizer: Tokenizer,
    config: Config,
    device: Device,
    dtype: DType,
    #[allow(dead_code)]
    arch: ModelArch,
}

impl Eu4AiModel {
    /// Load a model with LoRA adapter.
    ///
    /// Downloads the base model from HuggingFace Hub if not cached,
    /// then applies the LoRA adapter weights.
    pub fn load(config: ModelConfig) -> Result<Self> {
        let load_start = std::time::Instant::now();

        // Select device based on preference (auto-detects GPU)
        let device = select_device(config.device_pref);
        let dtype = config.dtype;

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

        // Load and optionally merge LoRA weights
        let vb = if !config.adapter_path.as_os_str().is_empty() {
            log::info!("Loading LoRA adapter from {:?}", config.adapter_path);

            // Load base weights into memory for merging
            log::info!("Loading base model weights for LoRA merge...");
            let base_weights = Self::load_base_weights(&weights_path, &device, dtype)?;

            // Load LoRA config and merge weights
            let lora_config = Self::load_lora_config(&config.adapter_path)?;
            let merged_weights = Self::merge_lora_weights(
                base_weights,
                &config.adapter_path,
                &lora_config,
                &device,
                dtype,
            )?;

            log::warn!("LoRA merge complete - creating model from merged weights");
            VarBuilder::from_tensors(merged_weights, dtype, &device)
        } else {
            // No adapter - use base model directly (memory-mapped for efficiency)
            log::info!("Loading base model weights...");
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], dtype, &device)? }
        };

        // Build the model
        let model = Llama::load(vb, &model_config).context("Failed to load Llama model")?;

        // Create KV cache
        let cache = Cache::new(true, dtype, &model_config, &device)?;

        let load_time = load_start.elapsed();
        log::warn!(
            "Model loaded in {:.2}s (device: {:?}, dtype: {:?})",
            load_time.as_secs_f64(),
            device,
            dtype
        );

        Ok(Self {
            model,
            cache,
            tokenizer,
            config: model_config,
            device,
            dtype,
            arch,
        })
    }

    /// Load LoRA config from adapter directory.
    fn load_lora_config(adapter_path: &Path) -> Result<LoraConfig> {
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

        Ok(lora_config)
    }

    /// Merge LoRA weights into base model weights.
    ///
    /// LoRA formula: W' = W + (B @ A) * (alpha / r)
    /// Where A is [r, in_features] and B is [out_features, r]
    fn merge_lora_weights(
        base_weights: HashMap<String, Tensor>,
        adapter_path: &Path,
        lora_config: &LoraConfig,
        device: &Device,
        dtype: DType,
    ) -> Result<HashMap<String, Tensor>> {
        let lora_weights_path = adapter_path.join("adapter_model.safetensors");
        let lora_data = std::fs::read(&lora_weights_path)
            .context("Failed to read adapter_model.safetensors")?;
        let lora_tensors =
            SafeTensors::deserialize(&lora_data).context("Failed to parse LoRA safetensors")?;

        // Log LoRA weight info
        let lora_keys: Vec<String> = lora_tensors.names().iter().map(|s| s.to_string()).collect();
        log::info!("LoRA adapter has {} weight tensors", lora_keys.len());
        log::debug!("LoRA keys: {:?}", &lora_keys[..lora_keys.len().min(10)]);

        // Scaling factor for LoRA
        let scale = lora_config.lora_alpha as f64 / lora_config.r as f64;
        log::info!("LoRA scale factor: {:.2}", scale);

        let mut merged = base_weights;
        let mut merge_count = 0;

        // Find LoRA A/B pairs and merge them with corresponding base weights
        // LoRA keys typically look like: base_model.model.layers.0.self_attn.q_proj.lora_A.weight
        // Base keys look like: model.layers.0.self_attn.q_proj.weight
        for lora_key in &lora_keys {
            if !lora_key.ends_with(".lora_A.weight") {
                continue;
            }

            // Extract the base name (e.g., "base_model.model.layers.0.self_attn.q_proj")
            let base_prefix = lora_key
                .strip_suffix(".lora_A.weight")
                .expect("Already checked suffix");

            // Construct B key
            let lora_b_key = format!("{}.lora_B.weight", base_prefix);
            if !lora_keys.iter().any(|k| k == &lora_b_key) {
                log::warn!("Missing LoRA B for {}", lora_key);
                continue;
            }

            // Map LoRA key to base model key
            // PEFT format: "base_model.model.model.layers.0.self_attn.q_proj"
            // Base model:  "model.layers.0.self_attn.q_proj.weight"
            let base_key = {
                let mut key = base_prefix.to_string();
                // Strip "base_model." prefix if present
                if let Some(rest) = key.strip_prefix("base_model.") {
                    key = rest.to_string();
                }
                // Strip extra "model." prefix if present (PEFT adds this)
                if let Some(rest) = key.strip_prefix("model.") {
                    key = rest.to_string();
                }
                format!("{}.weight", key)
            };

            // Check if base weight exists
            if !merged.contains_key(&base_key) {
                log::debug!("No base weight for LoRA target: {}", base_key);
                continue;
            }

            // Load LoRA tensors
            let lora_a_view = lora_tensors
                .tensor(lora_key)
                .context("Failed to get LoRA A")?;
            let lora_b_view = lora_tensors
                .tensor(&lora_b_key)
                .context("Failed to get LoRA B")?;

            // Convert to candle tensors
            let lora_a = Self::view_to_tensor(&lora_a_view, device, dtype)?;
            let lora_b = Self::view_to_tensor(&lora_b_view, device, dtype)?;

            log::debug!(
                "Merging {}: A{:?} x B{:?} into {:?}",
                base_key,
                lora_a.dims(),
                lora_b.dims(),
                merged[&base_key].dims()
            );

            // Compute delta = B @ A * scale
            // LoRA A is [r, in_features], B is [out_features, r]
            // delta = B @ A gives [out_features, in_features]
            let delta = lora_b.matmul(&lora_a)?;
            let delta = (delta * scale)?;

            // Merge: W' = W + delta
            let base_weight = merged.remove(&base_key).unwrap();
            let merged_weight = (&base_weight + &delta)?;
            merged.insert(base_key, merged_weight);
            merge_count += 1;
        }

        if merge_count > 0 {
            log::warn!("Merged {} LoRA weight pairs into base model", merge_count);
        } else {
            log::error!("No LoRA weights were merged! Check key mapping.");
        }
        Ok(merged)
    }

    /// Convert safetensors TensorView to candle Tensor.
    fn view_to_tensor(
        view: &safetensors::tensor::TensorView,
        device: &Device,
        dtype: DType,
    ) -> Result<Tensor> {
        let shape: Vec<usize> = view.shape().to_vec();
        let data = view.data();

        // SafeTensors stores in the dtype specified in the file
        // For PEFT LoRA, this is typically F32 or BF16
        let tensor = match view.dtype() {
            safetensors::Dtype::F32 => {
                let floats: &[f32] = bytemuck::cast_slice(data);
                Tensor::from_slice(floats, shape.as_slice(), device)?
            }
            safetensors::Dtype::F16 => {
                let halfs: &[half::f16] = bytemuck::cast_slice(data);
                let floats: Vec<f32> = halfs.iter().map(|h| h.to_f32()).collect();
                Tensor::from_slice(&floats, shape.as_slice(), device)?
            }
            safetensors::Dtype::BF16 => {
                let bhalfs: &[half::bf16] = bytemuck::cast_slice(data);
                let floats: Vec<f32> = bhalfs.iter().map(|h| h.to_f32()).collect();
                Tensor::from_slice(&floats, shape.as_slice(), device)?
            }
            other => anyhow::bail!("Unsupported LoRA dtype: {:?}", other),
        };

        // Convert to target dtype if needed
        if dtype != DType::F32 {
            tensor.to_dtype(dtype).context("Failed to convert dtype")
        } else {
            Ok(tensor)
        }
    }

    /// Load base model weights into a HashMap.
    fn load_base_weights(
        weights_path: &Path,
        device: &Device,
        dtype: DType,
    ) -> Result<HashMap<String, Tensor>> {
        let data = std::fs::read(weights_path).context("Failed to read model.safetensors")?;
        let tensors =
            SafeTensors::deserialize(&data).context("Failed to parse base safetensors")?;

        let mut weights = HashMap::new();
        for name in tensors.names() {
            let view = tensors.tensor(name)?;
            let tensor = Self::view_to_tensor(&view, device, dtype)?;
            weights.insert(name.to_string(), tensor);
        }

        log::info!("Loaded {} base model tensors", weights.len());
        Ok(weights)
    }

    /// Run inference on a prompt and return the chosen action index.
    ///
    /// The prompt should end with `<|choice|>` and the model will generate
    /// a single digit (0-9) representing the chosen action.
    pub fn choose_action(&mut self, prompt: &str) -> Result<usize> {
        let infer_start = std::time::Instant::now();

        // Log prompt at debug level (use --log-level debug to see full prompts)
        log::debug!(
            "=== LLM PROMPT ({} chars) ===\n{}\n=== END ===",
            prompt.len(),
            prompt
        );

        // Tokenize
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let tokens = encoding.get_ids();

        if tokens.is_empty() {
            anyhow::bail!("Tokenization produced empty sequence");
        }

        log::debug!("Tokenized to {} tokens", tokens.len());

        // Convert to tensor
        let input_ids = Tensor::new(tokens, &self.device)?.unsqueeze(0)?;

        // Create fresh cache for each inference (no clear() method available)
        let mut cache = Cache::new(true, self.dtype, &self.config, &self.device)
            .context("Failed to create KV cache")?;

        // Forward pass
        let logits = self
            .model
            .forward(&input_ids, 0, &mut cache)
            .context("Forward pass failed")?;

        // The model returns [batch, vocab] for the last position already
        // (or [batch, seq, vocab] for full sequence - check dimensions)
        let logits = if logits.dims().len() == 3 {
            let seq_len = logits.dim(1)?;
            logits.i((.., seq_len - 1, ..))?.squeeze(0)?
        } else {
            logits.squeeze(0)?
        };

        // Sample from digit tokens (0-9)
        let action = self.sample_digit(&logits)?;

        let infer_time = infer_start.elapsed();
        log::debug!(
            "LLM chose action {} in {:.0}ms ({} tokens)",
            action,
            infer_time.as_secs_f64() * 1000.0,
            tokens.len()
        );
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
