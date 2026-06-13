//! [`DetectionInput`]: arguments to [`Engine::detect`].
//!
//! [`Engine::detect`]: super::super::Engine::detect

use uuid::Uuid;

use crate::phases::ingestion::ImportFile;
use crate::pipeline::Plan;
use crate::policy::AnyPolicy;

/// Input required to execute a detection pass.
///
/// A detection pass runs imports → extraction → recognition →
/// deduplication → policy evaluation. It stops before applying
/// any redaction — the [`DetectionResult`] holds the audits with
/// their `Execution::Pending` decisions for the caller to review.
///
/// Export sinks live on the matching [`RedactionInput`] instead.
///
/// [`DetectionResult`]: super::DetectionResult
/// [`RedactionInput`]: super::super::redaction::RedactionInput
#[derive(Clone)]
pub struct DetectionInput {
    /// Identity of the human or service account initiating the run.
    pub actor_id: Uuid,
    /// Policies to apply, in precedence order: index `0` is highest
    /// precedence. Submitted inline with their full rule bodies —
    /// the engine does not persist policies as a resource. Callers
    /// reuse policies by re-submitting the same bytes.
    pub policies: Vec<AnyPolicy>,
    /// Content sources to ingest at the start of the run.
    pub imports: Vec<ImportFile>,
    /// Per-phase behaviour knobs the pipeline reads for each
    /// document.
    pub plan: Plan,
}
