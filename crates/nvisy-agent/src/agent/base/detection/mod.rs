//! Shared LLM-driven detection helpers.
//!
//! The LLM detect agents (`NerAgent`, `VlmAgent`) take their
//! per-document concerns — source payload, uploader hints, document
//! labels, correlation id — from the shared
//! [`RecognizerInput<M>`] surface instead of bespoke context structs.
//! No shared types live here today; the module exists as a future
//! home for cross-agent prompt utilities.
//!
//! [`RecognizerInput<M>`]: nvisy_core::RecognizerInput
