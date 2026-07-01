//! [`FileLineage`]: provenance for engine-produced files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Provenance for a [`FileMetadata`]. Uploaded files carry
/// `None`; engine-produced files (today: redaction apply
/// outputs) carry one of these variants so audits and clients
/// can trace any file back to what produced it.
///
/// [`FileMetadata`]: super::FileMetadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileLineage {
    /// Output of a redaction apply. `runId` is the run that
    /// produced this file; `sourceFileId` is the input file the
    /// run read.
    RedactedFrom {
        /// Run that produced this file (`/redactions/{runId}`).
        run_id: Uuid,
        /// Original input file the run read.
        source_file_id: Uuid,
    },
}
