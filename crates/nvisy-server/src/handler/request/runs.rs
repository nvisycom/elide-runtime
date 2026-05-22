//! Run request types.

use nvisy_engine::detection::Detection;
use nvisy_engine::extraction::Extraction;
use nvisy_engine::ingestion::{ExportFile, ImportFile};
use nvisy_engine::operation::{Deduplication, Validation};
use nvisy_engine::pipeline::{RunStatus, RuntimeConfig};
use nvisy_engine::redaction::Redaction;
use nvisy_ontology::policy::PolicyRef;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::Pagination;

/// Request body for `POST /runs`.
///
/// A flat, fixed-order pipeline plan. Each phase carries its own
/// config; the order is hardwired (extraction → detection →
/// deduplication → redaction → validation).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRun {
    /// Previously uploaded policies to apply, tagged with precedence.
    #[serde(default)]
    pub policies: Vec<PolicyRef>,
    /// Per-request configuration overrides (optional).
    #[serde(default)]
    #[schemars(skip)]
    pub config: Option<RuntimeConfig>,
    /// When `true`, evaluate detection and policy rules but skip
    /// validation and export. Returns the redaction plan without
    /// modifying or exporting content.
    #[serde(default)]
    pub dry_run: bool,

    /// Phase 0: content imports (source).
    #[serde(default)]
    pub imports: Vec<ImportFile>,
    /// Phase 0: context IDs to load into the cache.
    #[serde(default)]
    pub context_ids: Vec<Uuid>,

    /// Phase 1: extraction settings per modality.
    #[serde(default)]
    pub extraction: Extraction,
    /// Phase 2: detection settings (which recognizer kinds + hints).
    #[serde(default)]
    pub detection: Detection,
    /// Phase 3: deduplication settings.
    #[serde(default)]
    pub deduplication: Deduplication,
    /// Phase 4: redaction settings.
    #[serde(default)]
    pub redaction: Redaction,
    /// Phase 5: validation settings.
    #[serde(default)]
    pub validation: Validation,
    /// Phase 6: content exports (sink).
    #[serde(default)]
    pub exports: Vec<ExportFile>,
}

impl NewRun {
    /// Convert the request into an [`nvisy_engine::pipeline::EngineInput`]
    /// for the given actor.
    #[must_use]
    pub fn into_engine_input(self, actor_id: Uuid) -> nvisy_engine::pipeline::EngineInput {
        nvisy_engine::pipeline::EngineInput {
            actor_id,
            policies: self.policies,
            config: self.config,
            dry_run: self.dry_run,
            imports: self.imports,
            context_ids: self.context_ids,
            extraction: self.extraction,
            detection: self.detection,
            deduplication: self.deduplication,
            redaction: self.redaction,
            validation: self.validation,
            exports: self.exports,
        }
    }
}

/// Query parameters for listing runs.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunQuery {
    /// Filter by run status (e.g. `running`, `succeeded`).
    #[serde(default)]
    pub status: Option<RunStatus>,
    /// Pagination parameters.
    #[serde(flatten)]
    pub pagination: Pagination,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_engine_input_defaults_round_trip() {
        let actor_id = Uuid::new_v4();
        let new_run = NewRun::default();
        let input = new_run.into_engine_input(actor_id);

        assert_eq!(input.actor_id, actor_id);
        assert!(input.policies.is_empty());
        assert!(input.config.is_none());
        assert!(!input.dry_run);
        assert!(input.imports.is_empty());
        assert!(input.context_ids.is_empty());
        assert!(input.detection.kinds.is_empty());
        assert!(input.exports.is_empty());
    }

    #[test]
    fn into_engine_input_propagates_dry_run_and_imports() {
        let actor_id = Uuid::new_v4();
        let content_id = Uuid::new_v4();
        let new_run = NewRun {
            dry_run: true,
            imports: vec![ImportFile {
                content_ids: vec![content_id],
                ..Default::default()
            }],
            ..NewRun::default()
        };
        let input = new_run.into_engine_input(actor_id);

        assert!(input.dry_run);
        assert_eq!(input.imports.len(), 1);
        assert_eq!(input.imports[0].content_ids, vec![content_id]);
    }
}
