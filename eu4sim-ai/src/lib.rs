//! # EU4 AI Inference
//!
//! LLM-based AI player for EU4 simulation using Candle.
//!
//! Supports multiple model architectures (SmolLM2, Gemma) with LoRA adapters
//! for different AI personalities.

pub mod model;
pub mod prompt;

pub use model::{Eu4AiModel, ModelConfig};
pub use prompt::PromptBuilder;
