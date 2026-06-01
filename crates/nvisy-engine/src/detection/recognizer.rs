//! Object-safe per-modality recognizer traits.
//!
//! Each modality has its own trait — text recognizers consume a
//! [`DetectionContext`] and emit `Vec<Entity<Text>>`; image
//! recognizers consume a [`VlmDetectionContext`] and emit
//! `Vec<Entity<Image>>`. Both are object-safe so the engine can
//! store heterogeneous recognizers (built-in + user-registered) in a
//! single `Vec<Arc<dyn _>>` per modality.
//!
//! Names follow Presidio's class-name convention: each recognizer is
//! registered against a string identifier (built-ins use the
//! [`names`] constants), and per-plan filtering picks recognizers by
//! name from [`Detection::kinds`]. Unknown names are warn-logged at
//! dispatch and skipped, matching Presidio's lenient `entities=[...]`
//! semantics.
//!
//! [`DetectionContext`]: super::DetectionContext
//! [`VlmDetectionContext`]: super::VlmDetectionContext
//! [`Detection::kinds`]: super::Detection::kinds

use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Image, Text};

use super::context::{ImageDetectionContext, TextDetectionContext};

/// Stable registration names for the built-in recognizers.
///
/// Operators reference these in [`Detection::kinds`] plan filters and
/// `[detection.*]` config sections; we expose them as `const`s so
/// callers can avoid stringly-typed literals when they care to.
///
/// [`Detection::kinds`]: super::Detection::kinds
pub mod names {
    /// Pattern-based text recognizer (regex + dictionary + checksum).
    pub const PATTERN: &str = "pattern";
    /// NER-engine text recognizer.
    pub const NER: &str = "ner";
    /// LLM-backed text recognizer.
    pub const LLM: &str = "llm";
    /// VLM-backed image recognizer.
    pub const VLM: &str = "vlm";
}

/// Text-modality recognizer surface.
///
/// Implemented by every recognizer that scans text and emits
/// `Entity<Text>`. Object-safe — the engine stores
/// `Arc<dyn TextRecognizer>` for both built-ins and user-registered
/// custom recognizers.
#[async_trait::async_trait]
pub trait TextRecognizer: Send + Sync {
    /// Detect entities given the per-call context. Returned entities
    /// are in modality-local coordinates; the engine driver rebases
    /// them into document coordinates.
    async fn recognize(&self, ctx: &TextDetectionContext) -> Result<Vec<Entity<Text>>>;

    /// Reset per-document state. Default no-op — stateless
    /// recognizers don't need to override. LLM-backed recognizers
    /// override to clear cumulative usage trackers.
    async fn reset(&self) {}
}

/// Image-modality recognizer surface.
///
/// Implemented by every recognizer that scans images and emits
/// `Entity<Image>`. Object-safe — the engine stores
/// `Arc<dyn ImageRecognizer>` for both built-ins and user-registered
/// custom recognizers.
#[async_trait::async_trait]
pub trait ImageRecognizer: Send + Sync {
    /// Detect entities given the per-call context. Returned entities
    /// are in image-absolute coordinates; no per-block lifting is
    /// applied.
    async fn recognize(&self, ctx: &ImageDetectionContext) -> Result<Vec<Entity<Image>>>;

    /// Reset per-document state. Default no-op.
    async fn reset(&self) {}
}
