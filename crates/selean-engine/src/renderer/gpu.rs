//! GPU device initialization and management.
//!
//! Handles adapter selection with graceful fallback from high-performance
//! to low-power GPU, device creation with explicit feature requirements,
//! and diagnostic logging of the selected adapter.

use selean_common::error::EngineError;
use tracing::{info, warn};

/// Configuration for GPU context creation.
#[derive(Debug, Clone)]
pub struct GpuContextDescriptor {
    /// Label for the GPU device, used in debug tools and error messages.
    pub label: String,
    /// Maximum texture dimension supported. Defaults to the adapter's limit.
    /// Set this to constrain memory usage for texture atlases.
    pub max_texture_dimension: Option<u32>,
}

impl Default for GpuContextDescriptor {
    fn default() -> Self {
        Self {
            label: "Selean GPU".to_string(),
            max_texture_dimension: None,
        }
    }
}

/// Holds the initialized GPU device, queue, and adapter metadata.
///
/// This is the primary entry point for all GPU operations. Created once at
/// application startup and shared (via `Arc`) across rendering subsystems.
pub struct GpuContext {
    /// The wgpu device handle for creating GPU resources.
    pub device: wgpu::Device,
    /// The command queue for submitting GPU work.
    pub queue: wgpu::Queue,
    /// Information about the selected adapter (for diagnostics).
    pub adapter_info: wgpu::AdapterInfo,
}

impl GpuContext {
    /// Creates a new GPU context by selecting the best available adapter.
    ///
    /// Attempts `HighPerformance` power preference first, falling back to
    /// `LowPower` if no high-performance adapter is available. Logs the
    /// selected adapter's name, backend, and driver for diagnostics.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::NoAdapter` if no compatible GPU adapter is found.
    /// Returns `EngineError::DeviceCreation` if device creation fails.
    pub async fn new(descriptor: &GpuContextDescriptor) -> Result<Self, EngineError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = Self::select_adapter(&instance).await?;
        let adapter_info = adapter.get_info();

        info!(
            adapter = %adapter_info.name,
            backend = ?adapter_info.backend,
            driver = %adapter_info.driver,
            driver_info = %adapter_info.driver_info,
            "Selected GPU adapter"
        );

        let required_limits = Self::compute_limits(&adapter, descriptor);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some(&descriptor.label),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None, // No API trace path
            )
            .await
            .map_err(|e| EngineError::DeviceCreation {
                reason: e.to_string(),
            })?;

        info!(
            max_texture_2d = device.limits().max_texture_dimension_2d,
            max_buffer_size = device.limits().max_buffer_size,
            "GPU device created successfully"
        );

        Ok(Self {
            device,
            queue,
            adapter_info,
        })
    }

    /// Attempts adapter selection: high-performance first, then low-power fallback.
    async fn select_adapter(instance: &wgpu::Instance) -> Result<wgpu::Adapter, EngineError> {
        // Try high-performance (discrete GPU) first.
        if let Some(adapter) = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
        {
            return Ok(adapter);
        }

        warn!("No high-performance GPU adapter found, falling back to low-power");

        // Fall back to low-power (integrated GPU).
        if let Some(adapter) = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
        {
            return Ok(adapter);
        }

        Err(EngineError::NoAdapter {
            requested_backend: "any (tried HighPerformance and LowPower)".to_string(),
        })
    }

    /// Computes device limits, optionally capping texture dimensions.
    fn compute_limits(adapter: &wgpu::Adapter, descriptor: &GpuContextDescriptor) -> wgpu::Limits {
        let mut limits = adapter.limits();

        if let Some(max_tex) = descriptor.max_texture_dimension {
            limits.max_texture_dimension_2d = limits.max_texture_dimension_2d.min(max_tex);
        }

        limits
    }
}

impl std::fmt::Debug for GpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuContext")
            .field("adapter", &self.adapter_info.name)
            .field("backend", &self.adapter_info.backend)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_descriptor_has_label_and_no_texture_cap() {
        let desc = GpuContextDescriptor::default();
        assert_eq!(desc.label, "Selean GPU");
        assert!(desc.max_texture_dimension.is_none());
    }

    #[test]
    fn descriptor_custom_values() {
        let desc = GpuContextDescriptor {
            label: "Test GPU".to_string(),
            max_texture_dimension: Some(4096),
        };
        assert_eq!(desc.label, "Test GPU");
        assert_eq!(desc.max_texture_dimension, Some(4096));
    }

    #[test]
    fn descriptor_clone_preserves_fields() {
        let desc = GpuContextDescriptor {
            label: "Clone Test".to_string(),
            max_texture_dimension: Some(2048),
        };
        let cloned = desc.clone();
        assert_eq!(cloned.label, "Clone Test");
        assert_eq!(cloned.max_texture_dimension, Some(2048));
    }

    #[test]
    fn descriptor_debug_format() {
        let desc = GpuContextDescriptor::default();
        let debug = format!("{desc:?}");
        assert!(debug.contains("Selean GPU"));
        assert!(debug.contains("max_texture_dimension"));
    }
}
