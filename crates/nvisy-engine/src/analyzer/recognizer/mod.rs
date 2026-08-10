//! Per-recognizer compile helpers: `pattern`, `ner`, `llm`.
//!
//! Symmetric with [`super::enricher`]: each helper takes the
//! request's [`RecognizerParams`] and the deployment-owned config
//! for its lineup ([`NerConfig`] / [`LlmConfig`] for the two
//! inference lineups), and attaches the compiled recognizer to a
//! [`elide::detection::Analyzer<M>`].
//!
//! Pattern is modality-generic (`M: TextRecognizable`); NER and
//! LLM constrain over their upstream `Recognizer<M>` /
//! `LlmModality<M>` impls respectively — modalities that lack
//! the impl either fail the compile with a Validation error
//! (NER: cheap trait bound) or are silently skipped upstream
//! (LLM: no `LlmModality` impl for Tabular / Audio).
//!
//! [`NerConfig`]: crate::provider::ner::NerConfig
//! [`LlmConfig`]: crate::provider::llm::LlmConfig
//! [`RecognizerParams`]: nvisy_schema::plan::RecognizerParams

mod llm;
mod ner;
mod pattern;

pub(super) use self::llm::attach_llm_lineup;
pub(super) use self::ner::attach_ner_lineup;
pub(super) use self::pattern::attach as attach_pattern;
