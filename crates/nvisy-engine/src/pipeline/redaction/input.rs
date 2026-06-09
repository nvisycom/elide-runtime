//! [`RedactionInput`]: arguments to [`Engine::redact`].
//!
//! [`Engine::redact`]: super::super::Engine::redact

use uuid::Uuid;

use super::override_::RedactionOverride;
use crate::core::Plan;
use crate::phases::ingestion::ExportFile;

/// Input required to execute a redaction pass.
///
/// References a previously completed detection by id. The
/// detection's audits supply the entities and the operator each
/// policy chain picked; [`overrides`] lets the caller approve /
/// reject / replace specific decisions or add entities the
/// recognisers missed.
///
/// [`overrides`]: Self::overrides
#[derive(Clone)]
pub struct RedactionInput {
    /// Identity of the actor initiating the redaction. Must match
    /// the actor that owns the referenced detection.
    pub actor_id: Uuid,
    /// Detection pass to redact against.
    pub detection_id: Uuid,
    /// Per-entity decision overrides + added entities the
    /// recognisers missed. Empty means "apply the detection
    /// verbatim."
    pub overrides: Vec<RedactionOverride>,
    /// Per-phase behaviour knobs for the redaction / validation
    /// phases (validation thresholds, severity filters, etc.).
    pub plan: Plan,
    /// Sinks to write redacted content to.
    pub exports: Vec<ExportFile>,
}
