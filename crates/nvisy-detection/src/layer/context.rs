//! Processing-strategy markers for detection layers.
//!
//! A detection layer advertises its processing model via a
//! [`DetectionContext`] associated type.  The orchestrator inspects
//! the concrete context at the type level to decide whether to batch
//! all spans upfront or iterate one-by-one.

/// Marker trait for detection processing strategies.
pub trait DetectionContext: Send + Sync + 'static {}

/// All spans are collected upfront and processed independently.
///
/// The orchestrator gathers every span from the handler, then passes
/// them to [`Detect::detect`](super::Detect::detect) in a single call.
pub struct ParallelContext;
impl DetectionContext for ParallelContext {}

/// Spans are processed one at a time; the layer carries state between
/// calls.
///
/// The orchestrator feeds one span per invocation, allowing the layer
/// to accumulate context (e.g. prior text for NER sliding-window).
pub struct SequentialContext;
impl DetectionContext for SequentialContext {}
