//! CUDA-specific integration tests.
//!
//! These tests are skipped on systems without CUDA support.
//! They verify GPU-specific functionality:
//! 1. Large tensor operations are fast on GPU
//! 2. F16 dtype works correctly
//! 3. CPU ↔ GPU transfers work
//!
//! Run with: `cargo test -p eu4sim-ai --features cuda --test cuda_integration`

use candle_core::{DType, Device, Tensor};
use eu4sim_ai::cuda_available;

/// Skip test cleanly if CUDA is unavailable.
/// Returns the CUDA device if available.
fn require_cuda() -> Device {
    if !cuda_available() {
        eprintln!("Skipping CUDA test: no GPU available");
        std::process::exit(0); // Clean skip, test passes
    }
    Device::new_cuda(0).expect("CUDA device should be available")
}

#[test]
fn test_cuda_tensor_creation() {
    let device = require_cuda();

    // Create a reasonably large tensor on GPU
    let t = Tensor::zeros((1000, 1000), DType::F32, &device).unwrap();
    assert_eq!(t.dims(), &[1000, 1000]);

    // Verify we can read back
    let sum: f32 = t.sum_all().unwrap().to_scalar().unwrap();
    assert_eq!(sum, 0.0);
}

#[test]
fn test_cuda_large_matmul_performance() {
    let device = require_cuda();

    // Create large matrices
    let a = Tensor::randn(0.0f32, 1.0, (512, 512), &device).unwrap();
    let b = Tensor::randn(0.0f32, 1.0, (512, 512), &device).unwrap();

    // Time the matmul
    let start = std::time::Instant::now();
    let c = a.matmul(&b).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(c.dims(), &[512, 512]);
    println!("512x512 matmul on CUDA: {:?}", elapsed);

    // Should be reasonably fast on GPU (< 500ms even on modest GPUs)
    // This is a sanity check, not a strict benchmark
    assert!(
        elapsed.as_millis() < 1000,
        "CUDA matmul too slow: {:?}",
        elapsed
    );
}

#[test]
fn test_cuda_f16_operations() {
    let device = require_cuda();

    // F16 is crucial for efficient GPU inference
    let t = Tensor::ones((100, 100), DType::F16, &device).unwrap();
    assert_eq!(t.dtype(), DType::F16);

    // Verify we can do operations in F16
    let doubled = (&t + &t).unwrap();

    // Convert back to F32 for verification
    let as_f32 = doubled.to_dtype(DType::F32).unwrap();
    // Flatten and get first element
    let flat = as_f32.flatten_all().unwrap();
    let sample: f32 = flat.get(0).unwrap().to_scalar().unwrap();
    assert!((sample - 2.0).abs() < 0.01);
}

#[test]
fn test_cuda_bf16_operations() {
    let device = require_cuda();

    // BF16 is also important for some models
    let t = Tensor::ones((100, 100), DType::BF16, &device).unwrap();
    assert_eq!(t.dtype(), DType::BF16);

    // Verify conversion works
    let as_f32 = t.to_dtype(DType::F32).unwrap();
    let sum: f32 = as_f32.sum_all().unwrap().to_scalar().unwrap();
    assert!((sum - 10000.0).abs() < 1.0);
}

#[test]
fn test_cpu_to_gpu_transfer() {
    let gpu = require_cuda();

    // Create on CPU
    let cpu_tensor = Tensor::arange(0f32, 100.0, &Device::Cpu).unwrap();
    assert_eq!(cpu_tensor.dims(), &[100]);

    // Move to GPU
    let gpu_tensor = cpu_tensor.to_device(&gpu).unwrap();

    // Compute on GPU
    let squared = gpu_tensor.mul(&gpu_tensor).unwrap();

    // Move back to CPU for verification
    let back_to_cpu = squared.to_device(&Device::Cpu).unwrap();
    let vals: Vec<f32> = back_to_cpu.to_vec1().unwrap();

    assert_eq!(vals[0], 0.0); // 0^2
    assert_eq!(vals[1], 1.0); // 1^2
    assert_eq!(vals[10], 100.0); // 10^2
    assert!((vals[99] - 9801.0).abs() < 0.1); // 99^2
}

#[test]
fn test_cuda_batch_operations() {
    let device = require_cuda();

    // Simulate a batch of embeddings (common in LLM inference)
    let batch_size = 32;
    let seq_len = 128;
    let hidden_dim = 768;

    let embeddings = Tensor::randn(0.0f32, 1.0, (batch_size, seq_len, hidden_dim), &device).unwrap();
    let weights = Tensor::randn(0.0f32, 1.0, (hidden_dim, hidden_dim), &device).unwrap();

    // Reshape for matmul: [batch*seq, hidden] @ [hidden, hidden]
    let flat = embeddings.reshape((batch_size * seq_len, hidden_dim)).unwrap();
    let output = flat.matmul(&weights).unwrap();

    assert_eq!(output.dims(), &[batch_size * seq_len, hidden_dim]);

    // Reshape back
    let result = output.reshape((batch_size, seq_len, hidden_dim)).unwrap();
    assert_eq!(result.dims(), &[batch_size, seq_len, hidden_dim]);
}

#[test]
fn test_cuda_memory_doesnt_leak() {
    let device = require_cuda();

    // Create and drop many tensors
    for _ in 0..100 {
        let t = Tensor::randn(0.0f32, 1.0, (256, 256), &device).unwrap();
        let _ = t.sum_all().unwrap();
        // t is dropped here
    }

    // If we get here without OOM, memory management is working
    // Create one more large tensor to verify
    let final_t = Tensor::zeros((1024, 1024), DType::F32, &device).unwrap();
    assert_eq!(final_t.dims(), &[1024, 1024]);
}
