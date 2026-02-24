//! Processing-strategy markers for detection layers.
//!
//! A detection layer advertises its processing model via a
//! [`DetectionContext`] associated type.  The orchestrator inspects
//! the concrete context at the type level to decide whether to batch
//! all spans upfront or iterate one-by-one.
//!
//! The trait is **sealed** — only [`ParallelContext`] and
//! [`SequentialContext`] may implement it.  This guarantees the
//! orchestrator only needs to handle two calling conventions.

mod private {
    pub trait Sealed {}
}

/// Marker trait for detection processing strategies.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait DetectionContext: private::Sealed + Send + Sync + 'static {}

/// All spans are collected upfront and processed independently.
///
/// The orchestrator gathers every span from the handler, then passes
/// them to [`DetectionService::detect`](super::DetectionService::detect) in a single call.
pub struct ParallelContext;
impl private::Sealed for ParallelContext {}
impl DetectionContext for ParallelContext {}

/// Spans are processed one at a time; the layer carries state between
/// calls.
///
/// The orchestrator feeds one span per invocation, allowing the layer
/// to accumulate context (e.g. prior text for NER sliding-window).
pub struct SequentialContext;
impl private::Sealed for SequentialContext {}
impl DetectionContext for SequentialContext {}
