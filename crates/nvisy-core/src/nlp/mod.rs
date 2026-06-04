//! Shared NLP primitives stamped onto
//! [`RecognizerInput::artifacts`](crate::RecognizerInput::artifacts)
//! by an upstream `NlpEngine` and read by text recognizers + the
//! [`ContextEnhancer`].
//!
//! That `artifacts` field is an [`Artifacts`](crate::Artifacts)
//! newtype over [`type_map::concurrent::TypeMap`] — producers
//! insert one typed value per enrichment they computed, consumers
//! fetch by type and silently no-op when the enrichment wasn't
//! run. The shipped typed entries are:
//!
//! - [`LanguageDetections`] — languages the engine resolved (covers
//!   the whole doc, or per-span for multi-language docs).
//! - [`Tokens`] — tokenized text with optional lemmas; the
//!   `ContextEnhancer`'s lemma-aware matcher fetches this for
//!   morphological keyword recognition.
//! - [`StopwordSet`] — stopword list resolved for the dominant
//!   language.
//!
//! No NER spans are stamped here — recognizers that need NER hold
//! their own backends and do not participate in the shared pass.
//!
//! [`ContextEnhancer`]: crate::context::ContextEnhancer

mod languages;
mod stopwords;
mod token;

pub use self::languages::LanguageDetections;
pub use self::stopwords::StopwordSet;
pub use self::token::{Token, Tokens};
