//! Shared ORT runtime configuration for [`OrtBackend`] and
//! [`GlinerBackend`].
//!
//! Both constructors call [`auto_for_platform`] to pick an execution
//! provider matching the host OS + enabled Cargo features (CoreML on
//! macOS, CUDA on Linux/Windows, etc.), then pass the resulting
//! [`RuntimeParameters`] into the underlying `ort::Session` /
//! `gline-rs` model. No public override knob — operators tune by
//! enabling / disabling the corresponding feature at build time.
//!
//! [`OrtBackend`]: super::OrtBackend
//! [`GlinerBackend`]: super::GlinerBackend

pub(super) use orp::params::RuntimeParameters;
use ort::execution_providers::ExecutionProviderDispatch;

/// Build a [`RuntimeParameters`] with the host-appropriate execution
/// provider registered.
///
/// Selects whichever of the `coreml` / `cuda` / `tensorrt` /
/// `directml` features matches the current target OS, registering
/// one provider in the order they make sense for that platform. With
/// no matching feature enabled the result is a plain CPU runtime
/// (orp's default).
///
/// The chosen provider is registered as a *preferred* EP — ORT
/// itself silently falls back to CPU if the provider fails to
/// initialise at runtime (e.g. model uses an op the EP doesn't
/// support). The [`OrtBackend`] / [`GlinerBackend`] constructors
/// emit a `tracing::info!` log at startup naming the configured EP
/// so that silent fallback is at least visible in operator logs.
///
/// Crate-private: only called from `OrtParams::default()` /
/// `GlinerParams::default()`. Callers who want a different shape
/// build their own `RuntimeParameters`.
///
/// [`OrtBackend`]: super::OrtBackend
/// [`GlinerBackend`]: super::GlinerBackend
pub(super) fn auto_for_platform() -> RuntimeParameters {
    let providers = auto_execution_providers();
    if providers.is_empty() {
        RuntimeParameters::default()
    } else {
        RuntimeParameters::default().with_execution_providers(providers)
    }
}

/// Pick the execution provider(s) to register based on the host OS +
/// which `nvisy-nlp` Cargo features are enabled.
///
/// Priority order, first match wins per platform:
/// - macOS + `coreml` → CoreML
/// - non-macOS + `tensorrt` → TensorRT (NVIDIA, faster than raw CUDA)
/// - non-macOS + `cuda` → CUDA
/// - Windows + `directml` → DirectML
fn auto_execution_providers() -> Vec<ExecutionProviderDispatch> {
    vec![
        #[cfg(all(feature = "coreml", target_os = "macos"))]
        ort::execution_providers::CoreMLExecutionProvider::default().build(),
        #[cfg(all(feature = "tensorrt", not(target_os = "macos")))]
        ort::execution_providers::TensorRTExecutionProvider::default().build(),
        #[cfg(all(feature = "cuda", not(target_os = "macos")))]
        ort::execution_providers::CUDAExecutionProvider::default().build(),
        #[cfg(all(feature = "directml", target_os = "windows"))]
        ort::execution_providers::DirectMLExecutionProvider::default().build(),
    ]
}

/// Log the configured execution providers on backend construction so
/// operators can see at startup which EP is active (and which is
/// not). Falls back to CPU when the vec is empty.
pub(super) fn log_runtime(model_name: &str, runtime: &RuntimeParameters) {
    let providers = runtime.execution_providers();
    if providers.is_empty() {
        tracing::info!(
            target: "nvisy_nlp::ner",
            model = %model_name,
            threads = runtime.threads(),
            "NER backend loading on CPU (no execution providers registered)",
        );
    } else {
        tracing::info!(
            target: "nvisy_nlp::ner",
            model = %model_name,
            threads = runtime.threads(),
            execution_providers = ?providers,
            "NER backend loading with execution providers (silent CPU fallback possible if EP init fails)",
        );
    }
}
