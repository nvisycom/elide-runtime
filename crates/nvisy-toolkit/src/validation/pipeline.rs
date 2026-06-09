//! Ordered stack of [`Check`]s run head-to-tail.

use std::marker::PhantomData;

use nvisy_core::entity::Entity;
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::Modality;

use super::check::{Check, CheckContext, Finding};

/// Ordered stack of checks, run head-to-tail against an entity
/// slice.
///
/// Construction is open: callers compose any sequence of
/// [`Check<M, P>`] impls (built-in or custom).
///
/// `P` is the resolver type. Let the compiler infer it via `_` at
/// the call site.
pub struct CheckPipeline<M, P>
where
    M: Modality,
    P: TextAt<M> + ?Sized,
{
    checks: Vec<Box<dyn Check<M, P>>>,
    _marker: PhantomData<fn(&M, &P)>,
}

impl<M, P> CheckPipeline<M, P>
where
    M: Modality,
    P: TextAt<M> + ?Sized,
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

    /// Run every check in registration order against `entities` and
    /// collect their findings.
    pub async fn run(&self, entities: &[Entity<M>], ctx: &CheckContext<'_, M, P>) -> Vec<Finding> {
        let mut findings = Vec::new();
        for check in &self.checks {
            findings.extend(check.check(entities, ctx).await);
        }
        findings
    }
}

impl<M, P> Default for CheckPipeline<M, P>
where
    M: Modality,
    P: TextAt<M> + ?Sized,
{
    fn default() -> Self {
        Self::new()
    }
}
