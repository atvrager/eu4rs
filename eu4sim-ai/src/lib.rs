//! # EU4 AI Inference
//!
//! LLM-based AI player for EU4 simulation using Candle ML framework.
//!
//! ## Supported Models
//!
//! - **SmolLM2-360M** (default): Lightweight LLaMA-based model from HuggingFace
//! - **Gemma 2** (planned): Google's efficient language model
//!
//! ## LoRA Adapters
//!
//! Train custom adapters using the Colab notebook in `scripts/colab/`.
//! Adapters are automatically merged with base model weights at load time.
//!
//! ## Performance Benchmarks (CPU, F32)
//!
//! | Metric | SmolLM2-360M |
//! |--------|--------------|
//! | Model Load | ~1.0s |
//! | LoRA Merge | 160 weight pairs |
//! | Inference | 600-1000ms/prompt |
//! | Prompt Size | ~220-340 tokens |
//!
//! ## Usage
//!
//! ```bash
//! # Run simulation with LLM AI controlling top Great Power
//! cargo run -p eu4sim --release -- --observer --llm-ai models/adapter/run1 --ticks 100
//!
//! # With debug logging to see prompts
//! cargo run -p eu4sim --release -- --observer --llm-ai models/adapter/run1 --ticks 10 --log-level debug
//! ```

pub mod device;
pub mod llm_ai;
pub mod model;
pub mod prompt;

pub use device::{DevicePreference, cuda_available, select_device};
pub use llm_ai::LlmAi;
pub use model::{Eu4AiModel, ModelConfig};
pub use prompt::PromptBuilder;
