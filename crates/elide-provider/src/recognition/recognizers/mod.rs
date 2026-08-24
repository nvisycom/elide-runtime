//! Recognizers: the components that find entities.
//!
//! One module per backend, each holding its deployment
//! configuration — which model, which provider, which credentials
//! — beside nothing else. [`compile`] turns those lineups into the
//! recognizers elide runs.
//!
//! Split from [`super::enrichers`] by what a component *does*: a
//! recognizer produces entities, an enricher produces the context
//! recognizers read. Pattern recognition has no configuration (it
//! is elide's built-in catalogue) so it appears only in
//! [`compile`].

mod llm;
mod ner;

pub(crate) mod compile;

pub use self::llm::{
    AttachTo, AuthenticatedProvider, LlmBackend, LlmConfig, LlmSource, UnauthenticatedProvider,
};
pub use self::ner::{NerBackend, NerConfig};
