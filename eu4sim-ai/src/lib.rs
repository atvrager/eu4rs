//! # EU4 AI Inference
//!
//! LLM-based AI player for EU4 simulation using Candle.
//!
//! Supports multiple model architectures (SmolLM2, Gemma) with LoRA adapters
//! for different AI personalities.

pub mod llm_ai;
pub mod model;
pub mod prompt;

pub use llm_ai::LlmAi;
pub use model::{Eu4AiModel, ModelConfig};
pub use prompt::PromptBuilder;
