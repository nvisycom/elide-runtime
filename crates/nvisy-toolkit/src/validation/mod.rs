//! Composable post-redaction check pipeline.
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
//! Each check receives a `&[Entity<M>]` slice (the entities to
//! verify) plus a [`CheckContext`] carrying the resolver, the
//! optional post-redaction text, and an optional correlation id.
//! Callers that hold a richer carrier — `Document<M>` + audit —
//! adapt by passing the relevant entity slice in.
//!
//! [`leak`]: self::leak

mod check;
pub mod leak;
mod pipeline;

pub use self::check::{Check, CheckContext, Finding, FindingKind, Severity};
pub use self::leak::{CheckLeaks, LeakCheck, LeakFinding};
pub use self::pipeline::CheckPipeline;
