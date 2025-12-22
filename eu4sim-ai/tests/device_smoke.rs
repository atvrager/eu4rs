//! Smoke tests for device detection and basic tensor operations.
//!
//! These tests run on any platform (CPU fallback).
//! They verify that:
//! 1. Device selection never panics
//! 2. Basic tensor operations work on selected device
//! 3. Device detection reports correctly

use candle_core::{DType, Device, Tensor};
use eu4sim_ai::{DevicePreference, cuda_available, select_device};

#[test]
fn test_cpu_device_works() {
    let device = select_device(DevicePreference::CpuOnly);
    assert!(matches!(device, Device::Cpu));

    let t = Tensor::zeros((2, 3), DType::F32, &device).unwrap();
    assert_eq!(t.dims(), &[2, 3]);
}

#[test]
fn test_gpu_preferred_doesnt_panic() {
    // Should never panic — gracefully falls back to CPU if no GPU
    let device = select_device(DevicePreference::GpuPreferred);

    let t = Tensor::ones((4, 4), DType::F32, &device).unwrap();
    let sum: f32 = t.sum_all().unwrap().to_scalar().unwrap();
    assert_eq!(sum, 16.0);
}

#[test]
fn test_matmul_on_best_device() {
    let device = select_device(DevicePreference::GpuPreferred);

    // Create random matrices
    let a = Tensor::randn(0.0f32, 1.0, (64, 128), &device).unwrap();
    let b = Tensor::randn(0.0f32, 1.0, (128, 32), &device).unwrap();

    // Matrix multiply
    let c = a.matmul(&b).unwrap();
    assert_eq!(c.dims(), &[64, 32]);
}

#[test]
fn test_cuda_available_reports_correctly() {
    let available = cuda_available();
    println!("CUDA available: {}", available);

    // If CUDA is available, we should be able to create a CUDA device
    if available {
        let device = Device::new_cuda(0).expect("CUDA should work if reported available");
        assert!(matches!(device, Device::Cuda(_)));
    }
}

#[test]
fn test_tensor_dtype_conversions() {
    let device = select_device(DevicePreference::GpuPreferred);

    // Create F32 tensor
    let f32_tensor = Tensor::ones((10, 10), DType::F32, &device).unwrap();

    // Convert to F16 (important for GPU efficiency)
    let f16_tensor = f32_tensor.to_dtype(DType::F16).unwrap();
    assert_eq!(f16_tensor.dtype(), DType::F16);

    // Convert back
    let back = f16_tensor.to_dtype(DType::F32).unwrap();
    let sum: f32 = back.sum_all().unwrap().to_scalar().unwrap();
    assert!((sum - 100.0).abs() < 0.1);
}

#[test]
fn test_device_preference_default() {
    assert_eq!(DevicePreference::default(), DevicePreference::GpuPreferred);
}

#[test]
fn test_cuda_ordinal_fallback() {
    // Request a specific CUDA device that probably doesn't exist
    let device = select_device(DevicePreference::Cuda(99));

    // Should fall back to CPU without panicking
    // Just verify we got a usable device
    let _ = Tensor::zeros((1, 1), DType::F32, &device).unwrap();
}
