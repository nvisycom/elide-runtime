//! Post-redaction validation: composable check pipeline.
//!
//! Public surface:
//!
//! - [`Check`] / [`CheckContext`] / [`Finding`] / [`FindingKind`] /
//!   [`Severity`] — the abstract check abstraction shared by every
//!   validation pass.
//! - [`CheckPipeline`] — ordered stack of checks, built with
//!   `new().with_check(...).run(...)`.
//! - [`CheckLeaks`] / [`LeakCheck`] / [`LeakFinding`] — the canonical
//!   leak-detection implementation, scoped under the [`leak`]
//!   submodule. [`LeakCheck`] implements both [`CheckLeaks`] (for
//!   direct domain callers) and [`Check`] (so it slots into a
//!   pipeline).
//!
//! The phase orchestrator at [`ValidationPhase`] builds the
//! canonical pipeline from [`Validation`] for each
//! per-modality node and aggregates findings; any
//! [`Severity::Fail`] finding fails the run.
//!
//! [`Check`]: check::Check
//! [`CheckContext`]: check::CheckContext
//! [`Finding`]: check::Finding
//! [`FindingKind`]: check::FindingKind
//! [`Severity`]: check::Severity
//! [`CheckPipeline`]: pipeline::CheckPipeline
//! [`CheckLeaks`]: leak::CheckLeaks
//! [`LeakCheck`]: leak::LeakCheck
//! [`LeakFinding`]: leak::LeakFinding
//! [`ValidationPhase`]: crate::phases::validation::ValidationPhase
//! [`leak`]: self::leak

mod check;
pub mod leak;
mod pipeline;
mod plan;

pub use self::check::{Check, CheckContext, Finding, FindingKind, Severity};
pub use self::leak::{CheckLeaks, LeakCheck, LeakFinding};
pub use self::pipeline::CheckPipeline;
pub use self::plan::Validation;
