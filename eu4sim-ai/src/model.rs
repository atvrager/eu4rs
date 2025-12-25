//! Model loading and inference for EU4 AI.
//!
//! Loads SmolLM2, Gemma-3, or Gemma-2 base models from HuggingFace Hub,
//! applies LoRA adapters, and runs inference.

use crate::device::{DevicePreference, select_device};
use anyhow::{Context, Result};
use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::gemma3;
use candle_transformers::models::llama::{Cache, Config, Llama, LlamaConfig};
use hf_hub::{
    Repo, RepoType,
    api::sync::{Api, ApiBuilder},
};
use rand::Rng;
use safetensors::SafeTensors;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokenizers::Tokenizer;

/// Internal model representation supporting multiple architectures.
/// Size difference is acceptable - this enum is not copied frequently,
/// and boxing would add indirection overhead on the hot inference path.
#[allow(clippy::large_enum_variant)]
enum ModelInner {
    /// LLaMA-compatible models (SmolLM2, Gemma2)
    Llama {
        model: Llama,
        cache: Cache,
        config: Config,
    },
    /// Gemma 3 architecture (different cache handling)
    Gemma3 { model: gemma3::Model },
}

/// Supported model architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArch {
    /// SmolLM2 (LLaMA-like architecture)
    SmolLM2,
    /// Google Gemma 2
    Gemma2,
    /// Google Gemma 3 (270M-4B, newer architecture)
    Gemma3,
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
            "gemma3" | "gemma3_text" => Ok(Self::Gemma3),
            other => anyhow::bail!("Unknown model type: {}", other),
        }
    }

    /// Get the HuggingFace repo ID for the base model.
    pub fn base_model_repo(&self) -> &'static str {
        match self {
            Self::SmolLM2 => "HuggingFaceTB/SmolLM2-360M",
            Self::Gemma2 => "google/gemma-2-2b-it",
            Self::Gemma3 => "google/gemma-3-270m",
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

/// Gemma3 config as it appears in HuggingFace config.json.
/// Handles field name differences from candle's expected format.
#[derive(Debug, Clone, serde::Deserialize)]
struct Gemma3ConfigJson {
    pub attention_bias: bool,
    pub head_dim: usize,
    pub hidden_activation: String,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_attention_heads: usize,
    pub num_hidden_layers: usize,
    pub num_key_value_heads: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f64,
    pub rope_local_base_freq: f64,
    pub vocab_size: usize,
    pub final_logit_softcapping: Option<f64>,
    pub attn_logit_softcapping: Option<f64>,
    pub query_pre_attn_scalar: usize,
    pub sliding_window: usize,
    #[serde(rename = "_sliding_window_pattern")]
    pub sliding_window_pattern: usize,
    pub max_position_embeddings: usize,
}

impl Gemma3ConfigJson {
    /// Convert to candle's gemma3::Config.
    fn into_candle_config(self) -> gemma3::Config {
        use candle_nn::Activation;

        // Map activation string to enum
        let hidden_activation = match self.hidden_activation.as_str() {
            "gelu_pytorch_tanh" | "gelu" => Activation::Gelu,
            "silu" | "swiglu" => Activation::Silu,
            "relu" => Activation::Relu,
            other => {
                log::warn!("Unknown activation '{}', defaulting to Gelu", other);
                Activation::Gelu
            }
        };

        gemma3::Config {
            attention_bias: self.attention_bias,
            head_dim: self.head_dim,
            hidden_activation,
            hidden_size: self.hidden_size,
            intermediate_size: self.intermediate_size,
            num_attention_heads: self.num_attention_heads,
            num_hidden_layers: self.num_hidden_layers,
            num_key_value_heads: self.num_key_value_heads,
            rms_norm_eps: self.rms_norm_eps,
            rope_theta: self.rope_theta,
            rope_local_base_freq: self.rope_local_base_freq,
            vocab_size: self.vocab_size,
            final_logit_softcapping: self.final_logit_softcapping,
            attn_logit_softcapping: self.attn_logit_softcapping,
            query_pre_attn_scalar: self.query_pre_attn_scalar,
            sliding_window: self.sliding_window,
            sliding_window_pattern: self.sliding_window_pattern,
            max_position_embeddings: self.max_position_embeddings,
        }
    }
}

/// Unified model wrapper for EU4 AI inference.
pub struct Eu4AiModel {
    inner: ModelInner,
    tokenizer: Tokenizer,
    device: Device,
    dtype: DType,
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
        // Use HF_TOKEN from environment for authenticated access to gated models (e.g., Gemma)
        let api = if let Ok(token) = std::env::var("HF_TOKEN") {
            log::info!("Using HF_TOKEN for authenticated access");
            ApiBuilder::new()
                .with_token(Some(token))
                .build()
                .context("Failed to create authenticated HuggingFace API")?
        } else {
            Api::new().context("Failed to create HuggingFace API")?
        };
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

        // Load tokenizer (same for all architectures)
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Load model based on detected architecture
        let config_str = std::fs::read_to_string(&config_path).context("Failed to read config")?;

        let inner = match arch {
            ModelArch::SmolLM2 | ModelArch::Gemma2 => {
                // LLaMA-compatible loading path
                let llama_config: LlamaConfig =
                    serde_json::from_str(&config_str).context("Failed to parse LlamaConfig")?;
                let model_config = llama_config.into_config(false); // no flash attention

                // Load and optionally merge LoRA weights
                let vb = if !config.adapter_path.as_os_str().is_empty() {
                    log::info!("Loading LoRA adapter from {:?}", config.adapter_path);

                    let base_weights = Self::load_base_weights(&weights_path, &device, dtype)?;
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
                    log::info!("Loading base model weights...");
                    unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], dtype, &device)? }
                };

                let model = Llama::load(vb, &model_config).context("Failed to load Llama model")?;
                let cache = Cache::new(true, dtype, &model_config, &device)?;

                ModelInner::Llama {
                    model,
                    cache,
                    config: model_config,
                }
            }
            ModelArch::Gemma3 => {
                // Gemma 3 loading path (different architecture)
                // Use custom deserializer to handle HuggingFace config format differences
                let gemma_config_json: Gemma3ConfigJson =
                    serde_json::from_str(&config_str).context("Failed to parse Gemma3 config")?;
                let gemma_config = gemma_config_json.into_candle_config();

                // Load and optionally merge LoRA weights (same logic as LLaMA)
                let vb = if !config.adapter_path.as_os_str().is_empty() {
                    log::info!("Loading LoRA adapter from {:?}", config.adapter_path);

                    let base_weights = Self::load_base_weights(&weights_path, &device, dtype)?;
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
                    log::info!("Loading Gemma3 model weights...");
                    unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], dtype, &device)? }
                };

                let model = gemma3::Model::new(false, &gemma_config, vb)
                    .context("Failed to load Gemma3 model")?;

                ModelInner::Gemma3 { model }
            }
        };

        let load_time = load_start.elapsed();
        log::warn!(
            "Model loaded in {:.2}s (device: {:?}, dtype: {:?})",
            load_time.as_secs_f64(),
            device,
            dtype
        );

        Ok(Self {
            inner,
            tokenizer,
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

        // Validation: If we loaded keys but merged nothing, that's critical
        if !lora_keys.is_empty() && merge_count == 0 {
            anyhow::bail!(
                "LoRA merge failed: {} keys in adapter, 0 merged. Naming mismatch?",
                lora_keys.len()
            );
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

        // Forward pass - dispatch based on model architecture
        let logits = match &mut self.inner {
            ModelInner::Llama {
                model,
                cache,
                config,
            } => {
                // Create fresh cache for LLaMA (no clear() method available)
                *cache = Cache::new(true, self.dtype, config, &self.device)
                    .context("Failed to create KV cache")?;
                model
                    .forward(&input_ids, 0, cache)
                    .context("LLaMA forward pass failed")?
            }
            ModelInner::Gemma3 { model } => {
                // Gemma3 manages cache internally - clear before each inference
                model.clear_kv_cache();
                model
                    .forward(&input_ids, 0)
                    .context("Gemma3 forward pass failed")?
            }
        };

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

    /// Generate multi-action response using autoregressive sampling.
    ///
    /// Generates up to `max_tokens` tokens or until newline sequence completes
    /// all 6 categories.
    pub fn choose_multi_action(&mut self, prompt: &str, max_tokens: usize) -> Result<String> {
        let infer_start = std::time::Instant::now();

        log::debug!(
            "=== LLM PROMPT ({} chars) ===\n{}\n=== END ===",
            prompt.len(),
            prompt
        );

        // Tokenize prompt
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let mut tokens = encoding.get_ids().to_vec();

        if tokens.is_empty() {
            anyhow::bail!("Tokenization produced empty sequence");
        }

        log::debug!("Tokenized to {} tokens", tokens.len());

        let mut generated_text = String::new();

        // Autoregressive generation
        for step in 0..max_tokens {
            // Convert to tensor
            let input_ids = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;

            // Forward pass
            let logits = match &mut self.inner {
                ModelInner::Llama {
                    model,
                    cache,
                    config,
                } => {
                    // For incremental generation, use position = tokens.len() - 1
                    // and persistent cache
                    if step == 0 {
                        // First step: create fresh cache
                        *cache = Cache::new(true, self.dtype, config, &self.device)
                            .context("Failed to create KV cache")?;
                        model
                            .forward(&input_ids, 0, cache)
                            .context("LLaMA forward pass failed")?
                    } else {
                        // Incremental: only process last token
                        let last_token =
                            Tensor::new(&[tokens[tokens.len() - 1]], &self.device)?.unsqueeze(0)?;
                        model
                            .forward(&last_token, tokens.len() - 1, cache)
                            .context("LLaMA forward pass failed")?
                    }
                }
                ModelInner::Gemma3 { model } => {
                    // Gemma3 manages cache internally
                    if step == 0 {
                        model.clear_kv_cache();
                        // First step: process all tokens
                        model
                            .forward(&input_ids, 0)
                            .context("Gemma3 forward pass failed")?
                    } else {
                        // Incremental: only process last token
                        let last_token =
                            Tensor::new(&[tokens[tokens.len() - 1]], &self.device)?.unsqueeze(0)?;
                        model
                            .forward(&last_token, tokens.len() - 1)
                            .context("Gemma3 forward pass failed")?
                    }
                }
            };

            // Extract logits for last position
            let logits = if logits.dims().len() == 3 {
                let seq_len = logits.dim(1)?;
                logits.i((.., seq_len - 1, ..))?.squeeze(0)?
            } else {
                logits.squeeze(0)?
            };

            // Sample next token (greedy for determinism)
            let next_token = self.sample_greedy(&logits)?;
            tokens.push(next_token);

            // Decode and append
            if let Ok(text) = self.tokenizer.decode(&[next_token], false) {
                generated_text.push_str(&text);

                // Check for completion (all 6 categories present)
                if self.is_multi_action_complete(&generated_text) {
                    log::debug!("Multi-action complete after {} tokens", step + 1);
                    break;
                }
            }
        }

        let infer_time = infer_start.elapsed();
        log::debug!(
            "LLM generated {} chars in {:.0}ms ({} tokens)",
            generated_text.len(),
            infer_time.as_secs_f64() * 1000.0,
            generated_text.split_whitespace().count()
        );

        Ok(generated_text)
    }

    /// Check if multi-action response is complete (all 6 categories present).
    fn is_multi_action_complete(&self, text: &str) -> bool {
        let required = [
            "DIPLOMATIC:",
            "MILITARY:",
            "ECONOMIC:",
            "TRADE:",
            "COLONIZATION:",
            "OTHER:",
        ];
        required.iter().all(|cat| text.contains(cat))
    }

    /// Sample the most likely token (greedy decoding).
    fn sample_greedy(&self, logits: &Tensor) -> Result<u32> {
        let logits_vec: Vec<f32> = logits.to_vec1()?;
        let max_idx = logits_vec
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .ok_or_else(|| anyhow::anyhow!("Empty logits"))?;
        Ok(max_idx as u32)
    }

    /// Get the model architecture.
    pub fn arch(&self) -> ModelArch {
        self.arch
    }

    /// Get a reference to the tokenizer.
    pub fn tokenizer(&self) -> &Tokenizer {
        &self.tokenizer
    }

    /// Get the model config (LLaMA only, returns None for Gemma3).
    pub fn llama_config(&self) -> Option<&Config> {
        match &self.inner {
            ModelInner::Llama { config, .. } => Some(config),
            ModelInner::Gemma3 { .. } => None,
        }
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

        // Gemma 2 config
        std::fs::write(&config_path, r#"{"model_type": "gemma2"}"#).unwrap();
        assert_eq!(
            ModelArch::from_config(&config_path).unwrap(),
            ModelArch::Gemma2
        );

        // Gemma 3 config
        std::fs::write(&config_path, r#"{"model_type": "gemma3"}"#).unwrap();
        assert_eq!(
            ModelArch::from_config(&config_path).unwrap(),
            ModelArch::Gemma3
        );
    }
}
