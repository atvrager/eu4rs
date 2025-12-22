//! Device selection and detection for cross-platform ML inference.
//!
//! Provides runtime device detection with graceful fallback:
//! - CUDA (Linux/Windows with NVIDIA GPU)
//! - Metal (macOS with Apple Silicon)
//! - CPU (universal fallback)
//!
//! # Example
//!
//! ```ignore
//! use eu4sim_ai::device::{select_device, DevicePreference};
//!
//! // Auto-detect best available device
//! let device = select_device(DevicePreference::GpuPreferred);
//!
//! // Force CPU for testing
//! let cpu = select_device(DevicePreference::CpuOnly);
//! ```

use candle_core::Device;

/// Preferred device order for inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicePreference {
    /// Use GPU if available, fallback to CPU
    GpuPreferred,
    /// Force CPU (useful for testing/debugging)
    CpuOnly,
    /// Specific CUDA device index
    Cuda(usize),
}

impl Default for DevicePreference {
    fn default() -> Self {
        Self::GpuPreferred
    }
}

/// Detect and create the best available device.
///
/// Never panics — always returns a usable device (CPU as last resort).
pub fn select_device(pref: DevicePreference) -> Device {
    match pref {
        DevicePreference::CpuOnly => {
            log::info!("Device: CPU (forced)");
            Device::Cpu
        }
        DevicePreference::Cuda(ordinal) => match Device::new_cuda(ordinal) {
            Ok(dev) => {
                log::info!("Device: CUDA:{}", ordinal);
                dev
            }
            Err(e) => {
                log::warn!(
                    "CUDA:{} unavailable ({}), falling back to CPU",
                    ordinal,
                    e
                );
                Device::Cpu
            }
        },
        DevicePreference::GpuPreferred => {
            // Try CUDA first
            if let Ok(dev) = Device::new_cuda(0) {
                log::info!("Device: CUDA:0 (auto-detected)");
                return dev;
            }

            // Try Metal (macOS) — only compiled in when feature is enabled
            #[cfg(feature = "metal")]
            if let Ok(dev) = Device::new_metal(0) {
                log::info!("Device: Metal:0 (auto-detected)");
                return dev;
            }

            // Fallback to CPU
            log::info!("Device: CPU (no GPU available)");
            Device::Cpu
        }
    }
}

/// Check if CUDA is available (compile-time + runtime).
///
/// Returns `true` if:
/// 1. The `cuda` feature was enabled at compile time
/// 2. A CUDA device is present and functional at runtime
pub fn cuda_available() -> bool {
    Device::new_cuda(0).is_ok()
}

/// Check if Metal is available (macOS only).
///
/// Always returns false if compiled without the `metal` feature.
pub fn metal_available() -> bool {
    Device::new_metal(0).is_ok()
}

/// Get device info string for logging.
pub fn device_info(device: &Device) -> String {
    match device {
        Device::Cpu => "CPU".to_string(),
        Device::Cuda(_) => "CUDA".to_string(),
        Device::Metal(_) => "Metal".to_string(),
    }
}

/// Returns true if the device is a GPU (CUDA or Metal).
pub fn is_gpu(device: &Device) -> bool {
    !matches!(device, Device::Cpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_only_returns_cpu() {
        let device = select_device(DevicePreference::CpuOnly);
        assert!(matches!(device, Device::Cpu));
    }

    #[test]
    fn test_gpu_preferred_never_panics() {
        // Should always succeed, returning CPU if no GPU
        let device = select_device(DevicePreference::GpuPreferred);
        // Just verify we got something usable (CPU, CUDA, or Metal)
        assert!(!device_info(&device).is_empty());
    }

    #[test]
    fn test_device_info_cpu() {
        let info = device_info(&Device::Cpu);
        assert_eq!(info, "CPU");
    }

    #[test]
    fn test_is_gpu() {
        assert!(!is_gpu(&Device::Cpu));
        // Can't test CUDA without a GPU
    }

    #[test]
    fn test_default_is_gpu_preferred() {
        assert_eq!(DevicePreference::default(), DevicePreference::GpuPreferred);
    }
}
