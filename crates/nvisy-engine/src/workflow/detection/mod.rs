//! Detection node configuration.
//!
//! [`Detection`] (and every sub-params type it carries) lives in
//! `nvisy-detection`, next to the recognizers it configures.
//! The engine workflow simply re-exports the bundle so graph-node
//! deserialization and engine consumers see a single import path.
//!
//! `Detection::into_engine()` (on the detection crate) auto-assembles
//! a `DetectionEngine` with one recognizer per opted-in slot.

pub use crate::detection::{
    Detection, DetectionParams, LlmDetection, NerDetection, PatternDetection, PatternFilter,
};
