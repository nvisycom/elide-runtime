//! [`Check`]: the abstract validation pass.
//!
//! A [`Check`] inspects a single [`Document<M>`] post-redaction and
//! returns a list of [`Finding`]s. Checks are read-only — they
//! observe the document and the codec handle but never mutate the
//! audit records.
//!
//! Concrete check implementations live in submodules (today only
//! [`crate::validation::leak`]; future checks slot in as siblings).
//! Each domain typically defines its *own* trait
//! (e.g. [`CheckLeaks`]) carrying the domain-meaningful method, plus
//! a bridge `impl Check for ConcreteCheck` that wraps the domain
//! result into [`Finding`]s.
//!
//! Pipelines hold checks as `Box<dyn Check<M, P>>` and run them
//! head-to-tail; see [`CheckPipeline`].
//!
//! [`CheckLeaks`]: crate::validation::CheckLeaks
//! [`CheckPipeline`]: super::CheckPipeline
//! [`Document<M>`]: crate::document::Document

use std::marker::PhantomData;

use async_trait::async_trait;
use nvisy_core::TextAt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::document::Document;
use crate::modality::DocumentModality;

/// Read-only context every [`Check::check`] call receives.
///
/// Built once per node by the phase orchestrator and passed to each
/// check in the pipeline:
///
/// ```ignore
/// let ctx = CheckContext::new(&view, &handle).with_correlation_id(run_id);
/// pipeline.run(doc, &ctx).await;
/// ```
///
/// `P` is the resolver type, mirroring the dedup [`LayerContext`].
/// Generic so the resolver call (`ctx.resolver.text_at(...)`) is
/// monomorphised. Object safety on [`Check<M, P>`] still holds —
/// `P` is a type parameter, not a generic method.
///
/// [`LayerContext`]: nvisy_toolkit::deduplication::LayerContext
pub struct CheckContext<'a, M, P>
where
    M: DocumentModality,
    P: TextAt<M> + ?Sized,
{
    /// Resolver for "what value sits at this location?" Backed by
    /// `DocumentTree<M>` in production, mockable in tests.
    pub resolver: &'a P,
    /// Concatenated post-redaction output that checks like
    /// [`LeakCheck`] substring-search against. The validation phase
    /// streams the (already-redacted) handle chunks once and hands
    /// the assembled text in — checks never touch the codec
    /// directly.
    ///
    /// `None` when the modality doesn't produce searchable text
    /// (image / audio at present).
    ///
    /// [`LeakCheck`]: crate::validation::LeakCheck
    pub redacted_output: Option<&'a str>,
    /// Optional correlation id used to stitch tracing spans across
    /// the run.
    pub correlation_id: Option<Uuid>,
    /// Phantom binding `M` so the trait bound on `P` carries
    /// through without an unused-param error.
    _marker: PhantomData<&'a M>,
}

impl<'a, M, P> CheckContext<'a, M, P>
where
    M: DocumentModality,
    P: TextAt<M> + ?Sized,
{
    /// Build a context from the resolver alone. Checks that need the
    /// post-redaction text attach it via [`with_redacted_output`].
    ///
    /// [`with_redacted_output`]: Self::with_redacted_output
    pub fn new(resolver: &'a P) -> Self {
        Self {
            resolver,
            redacted_output: None,
            correlation_id: None,
            _marker: PhantomData,
        }
    }

    /// Attach the streamed post-redaction text the validation phase
    /// pulled out of the codec handle.
    pub fn with_redacted_output(mut self, redacted_output: &'a str) -> Self {
        self.redacted_output = Some(redacted_output);
        self
    }

    /// Attach a correlation id (typically a run id).
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

/// Severity of a single [`Finding`].
///
/// `Warn` causes the phase to log the finding and continue. `Fail`
/// causes the phase to log the finding and return a validation
/// error, failing the run.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Log + continue.
    #[default]
    Warn,
    /// Log + fail the run.
    Fail,
}

/// Discriminator on what *kind* of issue a [`Finding`] represents.
///
/// Each check kind extends this enum with its own variant. The enum
/// is `#[non_exhaustive]` so adding a new check kind doesn't break
/// existing match-on-FindingKind callers.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum FindingKind {
    /// A redacted value remained visible in the post-redaction
    /// output. Produced by [`LeakCheck`].
    ///
    /// [`LeakCheck`]: crate::validation::LeakCheck
    Leak {
        /// The entity whose redacted value was still found in the
        /// output.
        entity_id: Uuid,
        /// The original sensitive value that should have been
        /// redacted.
        value: String,
    },
    /// Catch-all for future check kinds. Carries an opaque
    /// human-readable message.
    Other,
}

/// One observation emitted by a [`Check`].
#[derive(Debug, Clone)]
pub struct Finding {
    /// Whether this finding should fail the run.
    pub severity: Severity,
    /// What kind of issue this finding represents.
    pub kind: FindingKind,
    /// Human-readable description of the issue, suitable for the
    /// tracing message and (on `Severity::Fail`) the error payload.
    pub message: String,
}

/// One stage of a validation pipeline.
///
/// Each check inspects `doc` and produces a list of findings.
/// Checks that find nothing return an empty vec; checks that don't
/// support the modality should not be registered in the first place
/// (the pipeline simply has no check for that modality).
#[async_trait]
pub trait Check<M, P>: Send + Sync
where
    M: DocumentModality,
    P: TextAt<M> + ?Sized,
{
    /// Inspect `doc` and emit a list of findings.
    async fn check(&self, doc: &Document<M>, ctx: &CheckContext<'_, M, P>) -> Vec<Finding>;
}
