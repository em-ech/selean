//! Unified error hierarchy for the Selean platform.
//!
//! Each crate defines its own domain-specific error type that wraps or
//! converts into the common error types defined here. Binary entry points
//! (server, Tauri app) may use `anyhow` to collect errors at the top level.

use thiserror::Error;

/// Top-level error type for Selean operations.
///
/// This enum covers errors that can originate from any layer of the system.
/// Crate-specific error types should convert into this via `From` implementations
/// when crossing crate boundaries.
#[derive(Debug, Error)]
pub enum SeleanError {
    /// An error originating from the rendering engine.
    #[error("engine error: {0}")]
    Engine(#[from] EngineError),

    /// An error from file I/O operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An error from serialization or deserialization.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// An operation was given invalid input.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A resource (node, token, component, etc.) was not found.
    #[error("{resource_type} not found: {id}")]
    NotFound {
        /// The type of resource that was not found (e.g., "Node", "Token").
        resource_type: &'static str,
        /// The string representation of the ID that was looked up.
        id: String,
    },
}

/// Errors specific to the rendering engine.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Failed to obtain a GPU adapter.
    #[error("no suitable GPU adapter found (requested: {requested_backend})")]
    NoAdapter {
        /// Description of the backend that was requested.
        requested_backend: String,
    },

    /// Failed to create a GPU device from the adapter.
    #[error("GPU device creation failed: {reason}")]
    DeviceCreation {
        /// Human-readable explanation of the failure.
        reason: String,
    },

    /// The rendering surface is invalid or lost.
    #[error("surface error: {reason}")]
    Surface {
        /// Human-readable explanation of the surface error.
        reason: String,
    },

    /// A shader compilation error.
    #[error("shader compilation failed: {reason}")]
    Shader {
        /// Human-readable explanation of the shader error.
        reason: String,
    },

    /// A required GPU feature is not supported by the adapter.
    #[error("unsupported GPU feature: {feature}")]
    UnsupportedFeature {
        /// Name of the unsupported feature.
        feature: String,
    },
}

/// A type alias for `Result` using `SeleanError`.
pub type Result<T> = std::result::Result<T, SeleanError>;

/// A type alias for `Result` using `EngineError`.
pub type EngineResult<T> = std::result::Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_error_display() {
        let err = EngineError::NoAdapter {
            requested_backend: "Vulkan".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "no suitable GPU adapter found (requested: Vulkan)"
        );
    }

    #[test]
    fn selean_error_from_engine_error() {
        let engine_err = EngineError::DeviceCreation {
            reason: "out of memory".to_string(),
        };
        let selean_err: SeleanError = engine_err.into();
        assert!(
            selean_err.to_string().contains("out of memory"),
            "Wrapped error should preserve message"
        );
    }

    #[test]
    fn not_found_error_display() {
        let err = SeleanError::NotFound {
            resource_type: "Node",
            id: "abc-123".to_string(),
        };
        assert_eq!(err.to_string(), "Node not found: abc-123");
    }

    #[test]
    fn io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let selean_err: SeleanError = io_err.into();
        assert!(selean_err.to_string().contains("file missing"));
    }

    #[test]
    fn all_engine_variants_display() {
        // Ensures every variant produces a non-empty display string.
        let variants: Vec<EngineError> = vec![
            EngineError::NoAdapter {
                requested_backend: "Metal".to_string(),
            },
            EngineError::DeviceCreation {
                reason: "limits exceeded".to_string(),
            },
            EngineError::Surface {
                reason: "lost".to_string(),
            },
            EngineError::Shader {
                reason: "syntax error".to_string(),
            },
            EngineError::UnsupportedFeature {
                feature: "TEXTURE_COMPRESSION_BC".to_string(),
            },
        ];

        for variant in variants {
            let msg = variant.to_string();
            assert!(!msg.is_empty(), "Error variant should have display text");
        }
    }
}
