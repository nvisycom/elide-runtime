//! Reusable multi-stage agent pipelines.
//!
//! A pipeline composes one or more agents (and any state shared
//! across calls) into a single end-to-end flow. Built-in pipelines:
//!
//! - [`LlmNerPipeline`] — text detection with [`NerAgent`] → verify
//!   with [`NerVerifyAgent`] → merge surviving candidates into
//!   coreference state.
//! - [`CvPipeline`] — image-side LLM work. Wraps a [`CvAgent`]
//!   (classifies pre-computed CV detections into entity categories)
//!   and a [`CvVerifyAgent`] (validates entity proposals against the
//!   image). Both flows are exposed as independent methods; callers
//!   that only need verification skip the agent side.
//!
//! Pipelines are concrete types — no shared trait. Each exposes its
//! own `run` / `classify` / `verify` methods plus `reset()` for
//! per-document state clearing and `usage()` for cumulative token
//! accounting. Callers that want bare agents instead can construct
//! them directly from [`agent`].
//!
//! [`agent`]: crate::agent
//! [`NerAgent`]: crate::agent::ner::NerAgent
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
//! [`CvAgent`]: crate::agent::cv::CvAgent
//! [`CvVerifyAgent`]: crate::agent::cv::CvVerifyAgent

mod cv;
mod llm_ner;

pub use self::cv::CvPipeline;
pub use self::llm_ner::LlmNerPipeline;
