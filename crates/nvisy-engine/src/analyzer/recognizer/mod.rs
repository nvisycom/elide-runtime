//! Per-recognizer compile helpers: `pattern`, `ner`, `llm`.
//!
//! Symmetric with [`super::enricher`]: each helper here takes a
//! per-recognizer spec from
//! [`nvisy_schema::plan::RecognizerParams`] plus whatever
//! deployment-side config the recognizer needs
//! (`NerConfig`/`LlmConfig` for those two lineups,
//! [`PatternGuardrails`] for the pattern recognizer), and
//! attaches the compiled recognizer to a
//! [`elide::detection::Analyzer<M>`].
//!
//! Pattern is modality-generic
//! (`M: TextRecognizable`); NER and LLM constrain over their
//! upstream `Recognizer<M>` / `LlmModality<M>` impls
//! respectively — modalities that lack the impl either fail the
//! compile with a Validation error (NER: cheap trait bound) or
//! are silently skipped upstream (LLM: no `LlmModality` impl for
//! Tabular / Audio).

mod guardrails;
mod llm;
mod ner;
mod pattern;

pub use self::guardrails::PatternGuardrails;
pub(super) use self::llm::attach_llm_lineup;
pub(super) use self::ner::attach_ner_lineup;
pub(super) use self::pattern::attach as attach_pattern;
