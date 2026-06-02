//! Shared NLP primitives: [`NlpArtifacts`] and the supporting types
//! that downstream crates consume.
//!
//! These are the platform-level shapes produced by an `NlpEngine`
//! (declared in `nvisy-ner`) and consumed by every text
//! [`Recognizer`](crate::Recognizer) plus the `ContextEnhancer`
//! (in `nvisy_core::context`, declared in a sibling module). Splitting them into this module —
//! instead of leaving them inside the producer crate — lets
//! `nvisy-pattern`, `nvisy_core::context`, and `nvisy-ner` all read
//! the same shapes without circular crate dependencies.
//!
//! # Conceptual model
//!
//! For every text scan, an `NlpEngine` produces one
//! [`NlpArtifacts`] value that bundles:
//!
//! - the language(s) the engine resolved
//!   ([`languages`](NlpArtifacts::languages)),
//! - the tokenized text with optional lemmas
//!   ([`tokens`](NlpArtifacts::tokens)),
//! - any NER spans the model predicted, in raw pre-normalization
//!   form ([`ner`](NlpArtifacts::ner)),
//! - the stopword set resolved for the dominant language
//!   ([`stopwords`](NlpArtifacts::stopwords)).
//!
//! The orchestrator wraps the artifacts in an `Arc` and hands the
//! same reference to every recognizer. Recognizers that don't need
//! artifacts (most patterns) ignore the field; recognizers that
//! require them (the NER adapter) read directly from
//! [`NlpArtifacts::ner`]; the post-recognition enhancer reads
//! [`NlpArtifacts::tokens`] for lemma-aware keyword matching.
//!
//! # Producer/consumer split
//!
//! - **Producer** (`nvisy-ner::nlp`): the `NlpEngine` trait + its
//!   built-in implementations (`LinguaNlpEngine` for language-only,
//!   `BentoNlpEngine` for full NLP via an external service) live in
//!   `nvisy-ner` because language-detection brings in the `lingua`
//!   dependency and Bento needs the http stack.
//! - **Consumer** (this module): the value types every NLP engine
//!   produces and every recognizer/enhancer reads. No engine
//!   implementations live here.

mod aggregation;
mod artifacts;
mod capabilities;
mod ner_span;
mod stopwords;
mod token;

pub use self::aggregation::{AggregationStrategy, AlignmentMode};
pub use self::artifacts::NlpArtifacts;
pub use self::capabilities::NlpCapabilities;
pub use self::ner_span::RawNerSpan;
pub use self::stopwords::StopwordSet;
pub use self::token::{Token, Tokens};
