//! Reusable multi-stage agent pipelines.
//!
//! A pipeline composes one or more agents into a single end-to-end
//! flow. Built-in pipelines:
//!
//! - [`LlmNerPipeline`] — text-side: [`NerAgent`] detect (with
//!   per-hint adjudication folded in) followed by an optional
//!   whole-audit [`NerVerifyAgent`] pass.
//! - [`VlmPipeline`] — image-side: [`VlmVerifyAgent`] confirms
//!   upstream entity proposals against the source image.
//!
//! Pipelines are concrete types — no shared trait. Each exposes
//! its own per-flow methods plus `reset()` for per-document state
//! clearing and `usage()` for cumulative token accounting. Callers
//! that want bare agents instead can construct them directly from
//! [`agent`].
//!
//! [`agent`]: crate::agent
//! [`NerAgent`]: crate::agent::ner::NerAgent
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
//! [`VlmVerifyAgent`]: crate::agent::vlm::VlmVerifyAgent

mod ner;
mod vlm;

pub use self::ner::LlmNerPipeline;
pub use self::vlm::VlmPipeline;
