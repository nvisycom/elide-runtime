//! [`FileMetadata`]: persisted descriptor for an uploaded file.
//!
//! A file in the engine is a `(metadata, bytes)` pair: the bytes
//! live in a blob-separated keyspace, the metadata in a small
//! JSON keyspace. The split lets `list_files` enumerate every
//! file for an actor without paying the cost of loading the
//! bytes. Both keyspaces key by `(actor_id, file_id)`.
//!
//! The descriptor mirrors what [`DocumentInput`] carries on a
//! [`StartBatch`] — the same `descriptor_labels` /
//! `descriptor_metadata` gate policies via
//! [`DocumentPredicate`]. When a run references a stored file,
//! the run inherits these gates.
//!
//! [`DocumentInput`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/runs/struct.DocumentInput.html
//! [`StartBatch`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/runs/struct.StartBatch.html
//! [`DocumentPredicate`]: crate::policy::DocumentPredicate

use std::collections::HashMap;

use hipstr::HipStr;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Descriptor for one stored file. Persisted as a small JSON
/// blob next to the raw bytes; clients consume this through
/// `GET /files` (list) and `GET /files/{id}` (one).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    /// Engine-minted UUIDv7. The `(actor_id, id)` pair keys both
    /// the metadata blob and the raw bytes.
    pub id: Uuid,
    /// Original filename, when the upload carried one (e.g. via
    /// `Content-Disposition: attachment; filename="scan.pdf"`).
    /// Display-only — the codec resolves on `extension`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub filename: Option<HipStr<'static>>,
    /// Caller-supplied MIME hint (e.g. `application/pdf`). The
    /// codec uses `extension`, not this; recorded for audit and
    /// for clients that round-trip metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub content_type: Option<HipStr<'static>>,
    /// File extension the codec registry resolves on (e.g.
    /// `"txt"`, `"pdf"`, `"png"`). Case-insensitive, no leading
    /// dot. Required at upload time — the engine derives it from
    /// `filename` when the upload omits it explicitly.
    #[schemars(with = "String")]
    pub extension: HipStr<'static>,
    /// Length of the raw bytes in bytes.
    pub size: u64,
    /// Hex-encoded SHA-256 of the raw bytes. Stable per upload —
    /// re-uploading the same bytes produces the same digest.
    #[schemars(with = "String")]
    pub digest: HipStr<'static>,
    /// Engine timestamp at upload time. UUIDv7's encoded
    /// timestamp also orders files, but this is the source of
    /// truth for display and filtering.
    #[schemars(with = "String")]
    pub uploaded_at: Timestamp,
    /// Where this file came from. `None` for uploaded files;
    /// [`FileLineage::RedactedFrom`] for files produced by a
    /// redaction apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<FileLineage>,
    /// Doc-level labels that gate
    /// [`DocumentPredicate::HasLabel`] policies when a run
    /// references this file.
    ///
    /// [`DocumentPredicate::HasLabel`]: crate::policy::DocumentPredicate::HasLabel
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptor_labels: Vec<String>,
    /// Doc-level metadata that gates
    /// [`DocumentPredicate::HasMetadata`] policies, merged with
    /// any per-request metadata at run-time.
    ///
    /// [`DocumentPredicate::HasMetadata`]: crate::policy::DocumentPredicate::HasMetadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub descriptor_metadata: HashMap<String, String>,
}

/// Provenance for a [`FileMetadata`]. Uploaded files carry
/// `None`; engine-produced files (today: redaction apply
/// outputs) carry one of these variants so audits and clients
/// can trace any file back to what produced it.
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
