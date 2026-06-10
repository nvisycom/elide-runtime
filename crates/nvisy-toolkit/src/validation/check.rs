//! Abstract [`Check`] trait + [`CheckContext`] + [`Finding`] /
//! [`Severity`] types every concrete check produces.

use std::marker::PhantomData;

use nvisy_core::entity::Entity;
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::Modality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Read-only context every [`Check::check`] call receives.
///
/// Built once per call by the caller and passed to each check in the
/// pipeline. `P` is the resolver type, mirroring the dedup
/// [`LayerContext`]. Generic so the resolver call
/// (`ctx.resolver.text_at(...)`) is monomorphised. Object safety on
/// [`Check<M, P>`] still holds — `P` is a type parameter, not a
/// generic method.
///
/// [`LayerContext`]: crate::deduplication::LayerContext
pub struct CheckContext<'a, M, P>
where
    M: Modality,
    P: TextAt<M> + ?Sized,
{
    /// Resolver for "what value sits at this location?"
    pub resolver: &'a P,
    /// Concatenated post-redaction output that checks like
    /// [`LeakCheck`] substring-search against. The caller streams the
    /// (already-redacted) handle chunks once and hands the assembled
    /// text in — checks never touch the codec directly.
    ///
    /// `None` when the modality doesn't produce searchable text
    /// (image / audio at present).
    ///
    /// [`LeakCheck`]: super::LeakCheck
    pub redacted_output: Option<&'a str>,
    /// Optional correlation id used to stitch tracing spans across
    /// the call.
    pub correlation_id: Option<Uuid>,
    _marker: PhantomData<&'a M>,
}

impl<'a, M, P> CheckContext<'a, M, P>
where
    M: Modality,
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

    /// Attach the streamed post-redaction text.
    pub fn with_redacted_output(mut self, redacted_output: &'a str) -> Self {
        self.redacted_output = Some(redacted_output);
        self
    }

    /// Attach a correlation id (typically a run / detection id).
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

/// Severity of a single [`Finding`].
///
/// `Warn` causes the caller to log the finding and continue. `Fail`
/// causes the caller to treat the result as a failure.
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
    /// Log + fail.
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
    /// [`LeakCheck`]: super::LeakCheck
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
    /// Whether this finding should fail the call.
    pub severity: Severity,
    /// What kind of issue this finding represents.
    pub kind: FindingKind,
    /// Human-readable description of the issue, suitable for the
    /// tracing message and (on `Severity::Fail`) the error payload.
    pub message: String,
}

/// One stage of a validation pipeline.
///
/// Each check inspects `entities` and produces a list of findings.
/// Checks that find nothing return an empty vec; checks that don't
/// support the modality should not be registered in the first place
/// (the pipeline simply has no check for that modality).
#[async_trait::async_trait]
pub trait Check<M, P>: Send + Sync
where
    M: Modality,
    P: TextAt<M> + ?Sized,
{
    /// Inspect `entities` and emit a list of findings.
    async fn check(&self, entities: &[Entity<M>], ctx: &CheckContext<'_, M, P>) -> Vec<Finding>;
}
