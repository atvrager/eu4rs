//! # EU4 AI Inference
//!
//! LLM-based AI player for EU4 simulation.
//!
//! ## Backends
//!
//! - **Candle** (default): Pure Rust inference, supports CUDA and Metal
//! - **Bridge**: Python inference server for ROCm (AMD GPU) support
//!
//! ## Supported Models
//!
//! - **SmolLM2-360M** (default): Lightweight LLaMA-based model from HuggingFace
//! - **Gemma-3-270M**: Google's compact model (6T tokens, 32K context)
//! - **Gemma 2**: Google's larger instruction-tuned model
//!
//! ## LoRA Adapters
//!
//! Train custom adapters using the scripts in `scripts/`.
//! Adapters are automatically merged with base model weights at load time.
//!
//! ## Performance Benchmarks
//!
//! | Backend | Device | Inference Time |
//! |---------|--------|----------------|
//! | Candle | CPU | 600-1000ms |
//! | Candle | CUDA | ~50-100ms |
//! | Bridge | ROCm | ~30-80ms |
//!
//! ## Usage
//!
//! ```bash
//! # Run with Candle backend (CPU/CUDA)
//! cargo run -p eu4sim --release -- --observer --llm-ai models/adapter/run1 --ticks 100
//!
//! # Run with Bridge backend (ROCm)
//! # First start the inference server:
//! #   cd scripts && python inference_server.py --adapter ../models/adapter
//! # Then run the simulation:
//! cargo run -p eu4sim --release -- --observer --llm-ai bridge --ticks 100
//! ```

pub mod bridge;
pub mod device;
pub mod llm_ai;
pub mod model;
pub mod prompt;

pub use bridge::{BridgeClient, BridgeServer, DEFAULT_HOST, DEFAULT_PORT};
pub use device::{DevicePreference, cuda_available, select_device};
pub use llm_ai::{InferenceBackend, LlmAi, LlmMessage};
pub use model::{Eu4AiModel, ModelConfig};
pub use prompt::PromptBuilder;
