//! Document-side validation glue: per-plan [`Validation`] config
//! plus convenient re-exports of the toolkit-side check API the
//! phase orchestrator and downstream callers use.
//!
//! The composable check abstraction (`Check`, `CheckPipeline`,
//! `CheckLeaks`, `LeakCheck`, …) lives in
//! [`nvisy_toolkit::validation`] — callers can craft their own
//! check pipelines without depending on `nvisy-document`. This
//! module re-exports those types for convenience and keeps the
//! per-plan [`Validation`] knob the document pipeline reads.

mod plan;

pub use nvisy_toolkit::validation::{
    Check, CheckContext, CheckLeaks, CheckPipeline, Finding, FindingKind, LeakCheck, LeakFinding,
    Severity,
};

pub use self::plan::Validation;
