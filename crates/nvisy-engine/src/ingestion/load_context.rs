//! Reference-data context loading.
//!
//! [`LoadContext`] is the context side of ingestion: it names the
//! reference-data contexts to load from the registry into the per-run
//! cache before any phase executes. Downstream detection and
//! redaction phases read those contexts from the cache.
//!
//! See [`ImportFile`] for the content side of ingestion.
//!
//! [`ImportFile`]: crate::ingestion::ImportFile

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Reference-data contexts to load into the per-run cache.
///
/// Each entry identifies a context by UUID. The engine resolves
/// every ID through the registry before the first phase runs;
/// downstream phases consume the loaded contexts directly.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Validate, Serialize, Deserialize, JsonSchema)]
pub struct LoadContext {
    /// Context identifiers to load. Must contain at least one.
    #[validate(length(min = 1, message = "load_context requires at least one context_id"))]
    pub context_ids: Vec<Uuid>,
}
