//! Deployment-owned recognizer provider configuration.
//!
//! The wire's `RecognizerParams.{ner,llm}` are three-state
//! toggles; every detail about which recognizers actually run
//! lives here, on the deployment's side. Operators pick model,
//! backend, and (future) credentials at deployment startup;
//! requests only opt in or out.

pub mod llm;
pub mod ner;
