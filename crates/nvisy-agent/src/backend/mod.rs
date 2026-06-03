//! Backend layer: the modality-agnostic [`LlmBackend`] trait that
//! turns a prompt + schema into the model's reply, plus its shipped
//! impls.
//!
//! Modality-specific work (prompt construction, response → entity
//! lifting) lives in [`crate::LlmRecognizer`]; backends only handle
//! provider dispatch, structured-output, retries, and usage
//! tracking.

mod llm_backend;
pub mod rig;

pub use self::llm_backend::{LlmBackend, LlmRequest, LlmResponse};
