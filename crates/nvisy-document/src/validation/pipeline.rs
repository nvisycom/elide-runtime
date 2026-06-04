//! [`CheckPipeline`]: ordered stack of [`Check`]s run head-to-tail.
//!
//! Mirrors the dedup [`LayerPipeline`] shape: build with
//! [`new`] + [`with_check`], run with [`run`].
//!
//! [`LayerPipeline`]: crate::deduplication::LayerPipeline
//! [`new`]: CheckPipeline::new
//! [`with_check`]: CheckPipeline::with_check
//! [`run`]: CheckPipeline::run

use std::marker::PhantomData;

use nvisy_core::ValueAt;

use super::check::{Check, CheckContext, Finding};
use crate::document::Document;
use crate::modality::DocumentModality;

/// Ordered stack of checks, run head-to-tail against a document.
///
/// Construction is open: callers compose any sequence of
/// [`Check<M, P>`] impls (built-in or custom). The canonical leak
/// recipe is built by the phase orchestrator from
/// [`Validation`] each call.
///
/// [`Validation`]: super::Validation
///
/// `P` is the resolver type, typically `DocumentView<'_, M>` at the
/// production call site (let the compiler infer it via `_`).
pub struct CheckPipeline<M, P>
where
    M: DocumentModality,
    P: ValueAt<M> + ?Sized,
{
    checks: Vec<Box<dyn Check<M, P>>>,
    _marker: PhantomData<fn(&M, &P)>,
}

impl<M, P> CheckPipeline<M, P>
where
    M: DocumentModality,
    P: ValueAt<M> + ?Sized,
{
    /// Empty pipeline. Use [`Self::with_check`] to append checks.
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Append a check.
    pub fn with_check<C: Check<M, P> + 'static>(mut self, check: C) -> Self {
        self.checks.push(Box::new(check));
        self
    }

    /// Run every check in registration order against `doc` and
    /// collect their findings.
    pub async fn run(&self, doc: &Document<M>, ctx: &CheckContext<'_, M, P>) -> Vec<Finding> {
        let mut findings = Vec::new();
        for check in &self.checks {
            findings.extend(check.check(doc, ctx).await);
        }
        findings
    }
}

impl<M, P> Default for CheckPipeline<M, P>
where
    M: DocumentModality,
    P: ValueAt<M> + ?Sized,
{
    fn default() -> Self {
        Self::new()
    }
}
