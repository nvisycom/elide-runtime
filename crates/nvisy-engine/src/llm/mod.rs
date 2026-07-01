//! LLM compile layer: takes the deployment's [`LlmConfig`] and a
//! per-modality analyzer, attaches every configured LLM recognizer
//! whose modality list includes the analyzer's modality.
//!
//! Consumed exclusively by [`super::engine::analyzer`]'s per-modality
//! compile functions when `AnalyzerParams.recognizers.llm == true`.
//! When the toggle is `false` this module is not called at all;
//! when it's `true` but the config has no matching recognizers,
//! the caller (analyzer compile) surfaces a `Validation` error.
//!
//! [`LlmConfig`]: nvisy_core::llm::LlmConfig

mod attach;

pub(crate) use self::attach::attach_lineup;
