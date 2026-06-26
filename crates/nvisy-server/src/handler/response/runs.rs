//! Run + per-document response wrappers.
//!
//! Engine types ([`nvisy_engine::runs::Run`],
//! [`RunDocument`], [`DocBody`], [`EntityRecord<M>`]) don't
//! derive `JsonSchema`. The server defines flat per-modality
//! mirror DTOs so the OpenAPI spec fully describes the wire
//! shape, with `From` conversions from the engine types.
//!
//! Provenance (the per-recognizer audit trail) and
//! `recognized_range` are intentionally dropped from the wire —
//! they're audit-internal and not on the review UI's critical
//! path. Add them back when there's a consumer.

use std::collections::HashMap;

use elide_core::entity::Entity;
use elide_core::modality::audio::{Audio, AudioLocation};
use elide_core::modality::image::{Image, ImageLocation};
use elide_core::modality::tabular::{Tabular, TabularLocation};
use elide_core::modality::text::{Text, TextLocation};
use jiff::Timestamp;
use nvisy_core::policy::RuleAction;
use nvisy_engine::runs::{
    DocBody, EntityRecord, ModalityKind, Run, RunDocState, RunDocument, RunState,
};
use schemars::JsonSchema;
use semver::Version;
use serde::Serialize;
use uuid::Uuid;

/// Run header + every per-document body, packaged as one
/// response. `GET /detections/{id}` and `GET /redactions/{id}`
/// both render through this — the caller filters by
/// [`state`](RunResponse::state) to know which view they're in.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    /// Run id (same as the detection id and the redaction id —
    /// detections and redactions are filtered views of the same
    /// underlying run).
    pub id: Uuid,
    /// Top-level run state.
    pub state: RunStateDto,
    /// Detail when [`state`](Self::state) is
    /// [`RunStateDto::Failed`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// UUIDv7 timestamp the run was started.
    #[schemars(with = "String")]
    pub started_at: Timestamp,
    /// UUIDv7 timestamp of the most recent state transition.
    #[schemars(with = "String")]
    pub updated_at: Timestamp,
    /// Policies the caller submitted.
    pub policy_refs: Vec<ResourceRefDto>,
    /// Contexts the caller submitted.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_refs: Vec<ResourceRefDto>,
    /// Per-request metadata.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    /// Per-document state. One entry per input file in the run.
    pub documents: Vec<RunDocumentDto>,
}

/// Wire-format mirror of [`RunState`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStateDto {
    /// Analyze in flight or queued.
    Analyzing,
    /// Analyze finished; awaiting reviewer + apply.
    AwaitingReview,
    /// Apply ran; every document succeeded.
    Applied,
    /// Apply ran; some documents succeeded, others failed.
    PartiallyApplied,
    /// Run failed before producing per-doc state worth
    /// reviewing; the [`failure_reason`](RunResponse::failure_reason)
    /// field carries the detail.
    Failed,
}

/// Wire-format mirror of [`nvisy_engine::runs::ResourceRef`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRefDto {
    /// Resource UUID.
    pub id: Uuid,
    /// Resource version.
    #[schemars(with = "String")]
    pub version: Version,
}

/// Wire-format mirror of [`RunDocument`]. Inlines the
/// per-modality body so the response is one flat array of docs
/// rather than a header + indirected fetches.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunDocumentDto {
    /// Doc id within the run.
    pub id: Uuid,
    /// Input file the doc analyzed.
    pub input_file_id: Uuid,
    /// Redacted output file, when apply succeeded for this doc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_id: Option<Uuid>,
    /// Per-doc lifecycle state.
    pub state: RunDocStateDto,
    /// Detail when [`state`](Self::state) is
    /// [`RunDocStateDto::Failed`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Modality the codec resolved this doc to.
    pub modality: ModalityDto,
    /// Recognized entities + reviewer overrides. Variant
    /// matches [`modality`](Self::modality).
    pub body: DocBodyDto,
}

/// Wire-format mirror of [`RunDocState`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunDocStateDto {
    /// Awaiting its turn on the analyze semaphore.
    Queued,
    /// Analyze in flight.
    Analyzing,
    /// Analyze finished; awaiting reviewer + apply.
    AwaitingReview,
    /// Apply ran for this doc; the redacted output is in
    /// [`output_file_id`](RunDocumentDto::output_file_id).
    Applied,
    /// Analyze or apply errored.
    Failed,
    /// Per-doc timeout fired.
    TimedOut,
}

/// Wire-format mirror of [`ModalityKind`].
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModalityDto {
    /// Text.
    Text,
    /// Tabular.
    Tabular,
    /// Image.
    Image,
    /// Audio.
    Audio,
}

/// Per-modality body of recognized entities + reviewer
/// overrides. Tagged by `modality`; variant matches
/// [`RunDocumentDto::modality`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum DocBodyDto {
    /// Text entities.
    Text {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<TextEntityRecordDto>,
    },
    /// Tabular entities.
    Tabular {
        /// Recognized entities.
        entities: Vec<TabularEntityRecordDto>,
    },
    /// Image entities.
    Image {
        /// Recognized entities.
        entities: Vec<ImageEntityRecordDto>,
    },
    /// Audio entities.
    Audio {
        /// Recognized entities.
        entities: Vec<AudioEntityRecordDto>,
    },
}

/// Shared scalar fields every per-modality entity record
/// carries.
fn entity_common<M>(entity: &Entity<M>) -> EntityCommon
where
    M: elide_core::modality::Modality,
{
    EntityCommon {
        id: entity.id,
        label: entity.label.as_str().to_owned(),
        confidence: f32::from(entity.confidence),
        coref: entity.coref.as_ref().map(|c| c.as_str().to_owned()),
        language: entity.language.as_ref().map(|l| l.as_str().to_owned()),
    }
}

/// Scalar-only fields shared across the four
/// per-modality entity DTOs. Inlined into each via
/// `#[serde(flatten)]`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityCommon {
    /// Stable per-entity id.
    pub id: Uuid,
    /// Label name (e.g. `"person_name"`, `"email_address"`).
    pub label: String,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f32,
    /// Coreference cluster id, when resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coref: Option<String>,
    /// Detected language, when resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Wire-format mirror of [`EntityRecord<Text>`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextEntityRecordDto {
    /// Shared scalars.
    #[serde(flatten)]
    pub common: EntityCommon,
    /// Byte range in the source text.
    pub location: TextLocationDto,
    /// Reviewer override action, when set.
    #[serde(skip_serializing_if = "Option::is_none", rename = "override")]
    pub r#override: Option<RuleAction>,
}

/// Wire-format mirror of [`TextLocation`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextLocationDto {
    /// Byte offset where the range starts.
    pub start: usize,
    /// Byte offset where the range ends (exclusive).
    pub end: usize,
    /// 1-based page number, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

/// Wire-format mirror of [`EntityRecord<Tabular>`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularEntityRecordDto {
    /// Shared scalars.
    #[serde(flatten)]
    pub common: EntityCommon,
    /// Cell coordinates.
    pub location: TabularLocationDto,
    /// Reviewer override action, when set.
    #[serde(skip_serializing_if = "Option::is_none", rename = "override")]
    pub r#override: Option<RuleAction>,
}

/// Wire-format mirror of [`TabularLocation`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularLocationDto {
    /// Zero-based row index.
    pub row_index: u32,
    /// Zero-based column index.
    pub column_index: u32,
    /// Byte offset within the cell where the entity starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<usize>,
    /// Byte offset within the cell where the entity ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<usize>,
    /// Column name from the header row, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
    /// Sheet name, for multi-sheet sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
}

/// Wire-format mirror of [`EntityRecord<Image>`]. Image
/// location is the entity's bounding box; the polygon variant
/// from elide is reduced to its bounding box in the wire format.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageEntityRecordDto {
    /// Shared scalars.
    #[serde(flatten)]
    pub common: EntityCommon,
    /// Axis-aligned bounding box in pixel coordinates.
    pub location: ImageLocationDto,
    /// Reviewer override action, when set.
    #[serde(skip_serializing_if = "Option::is_none", rename = "override")]
    pub r#override: Option<RuleAction>,
}

/// Wire-format mirror of the image bounding box. Coordinates
/// are floats matching elide's internal precision.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageLocationDto {
    /// X coordinate of the box's top-left corner.
    pub x: f64,
    /// Y coordinate of the box's top-left corner.
    pub y: f64,
    /// Box width.
    pub width: f64,
    /// Box height.
    pub height: f64,
    /// 1-based page number for multi-page documents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

/// Wire-format mirror of [`EntityRecord<Audio>`]. Audio
/// location is a time span over the source stream.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioEntityRecordDto {
    /// Shared scalars.
    #[serde(flatten)]
    pub common: EntityCommon,
    /// Time span on the source stream.
    pub location: AudioLocationDto,
    /// Reviewer override action, when set.
    #[serde(skip_serializing_if = "Option::is_none", rename = "override")]
    pub r#override: Option<RuleAction>,
}

/// Wire-format mirror of [`AudioLocation`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioLocationDto {
    /// Start of the span, in milliseconds from the stream
    /// origin.
    pub start_ms: u64,
    /// End of the span, in milliseconds from the stream origin.
    pub end_ms: u64,
    /// Diarization label of the speaker, when a diarizer
    /// assigned one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
}

// ---- Conversions ----

impl RunResponse {
    /// Assemble a response from the run header + the per-doc
    /// rows the caller fetched (one fjall round-trip per id).
    pub fn assemble(run: Run, documents: Vec<RunDocument>) -> Self {
        let (state, failure_reason) = match run.state {
            RunState::Analyzing => (RunStateDto::Analyzing, None),
            RunState::AwaitingReview => (RunStateDto::AwaitingReview, None),
            RunState::Applied => (RunStateDto::Applied, None),
            RunState::PartiallyApplied => (RunStateDto::PartiallyApplied, None),
            RunState::Failed { reason } => (RunStateDto::Failed, Some(reason)),
        };
        RunResponse {
            id: run.id,
            state,
            failure_reason,
            started_at: run.started_at,
            updated_at: run.updated_at,
            policy_refs: run.policy_refs.into_iter().map(Into::into).collect(),
            context_refs: run.context_refs.into_iter().map(Into::into).collect(),
            metadata: run.metadata,
            documents: documents.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<nvisy_engine::runs::ResourceRef> for ResourceRefDto {
    fn from(r: nvisy_engine::runs::ResourceRef) -> Self {
        ResourceRefDto {
            id: r.id,
            version: r.version,
        }
    }
}

impl From<RunDocument> for RunDocumentDto {
    fn from(doc: RunDocument) -> Self {
        let (state, failure_reason) = match doc.state {
            RunDocState::Queued => (RunDocStateDto::Queued, None),
            RunDocState::Analyzing => (RunDocStateDto::Analyzing, None),
            RunDocState::AwaitingReview => (RunDocStateDto::AwaitingReview, None),
            RunDocState::Applied => (RunDocStateDto::Applied, None),
            RunDocState::Failed { reason } => (RunDocStateDto::Failed, Some(reason)),
            RunDocState::TimedOut => (RunDocStateDto::TimedOut, None),
        };
        RunDocumentDto {
            id: doc.id,
            input_file_id: doc.input_file_id,
            output_file_id: doc.output_file_id,
            state,
            failure_reason,
            modality: doc.modality.into(),
            body: doc.body.into(),
        }
    }
}

impl From<ModalityKind> for ModalityDto {
    fn from(m: ModalityKind) -> Self {
        match m {
            ModalityKind::Text => ModalityDto::Text,
            ModalityKind::Tabular => ModalityDto::Tabular,
            ModalityKind::Image => ModalityDto::Image,
            ModalityKind::Audio => ModalityDto::Audio,
        }
    }
}

impl From<DocBody> for DocBodyDto {
    fn from(body: DocBody) -> Self {
        match body {
            DocBody::Text { entities } => DocBodyDto::Text {
                entities: entities.into_iter().map(Into::into).collect(),
            },
            DocBody::Tabular { entities } => DocBodyDto::Tabular {
                entities: entities.into_iter().map(Into::into).collect(),
            },
            DocBody::Image { entities } => DocBodyDto::Image {
                entities: entities.into_iter().map(Into::into).collect(),
            },
            DocBody::Audio { entities } => DocBodyDto::Audio {
                entities: entities.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<EntityRecord<Text>> for TextEntityRecordDto {
    fn from(record: EntityRecord<Text>) -> Self {
        let common = entity_common(&record.entity);
        TextEntityRecordDto {
            common,
            location: record.entity.location.into(),
            r#override: record.r#override,
        }
    }
}

impl From<TextLocation> for TextLocationDto {
    fn from(loc: TextLocation) -> Self {
        TextLocationDto {
            start: loc.start,
            end: loc.end,
            page: loc.page,
        }
    }
}

impl From<EntityRecord<Tabular>> for TabularEntityRecordDto {
    fn from(record: EntityRecord<Tabular>) -> Self {
        let common = entity_common(&record.entity);
        TabularEntityRecordDto {
            common,
            location: record.entity.location.into(),
            r#override: record.r#override,
        }
    }
}

impl From<TabularLocation> for TabularLocationDto {
    fn from(loc: TabularLocation) -> Self {
        TabularLocationDto {
            row_index: loc.row_index,
            column_index: loc.column_index,
            start_offset: loc.start_offset,
            end_offset: loc.end_offset,
            column_name: loc.column_name.map(|s| s.as_str().to_owned()),
            sheet_name: loc.sheet_name.map(|s| s.as_str().to_owned()),
        }
    }
}

impl From<EntityRecord<Image>> for ImageEntityRecordDto {
    fn from(record: EntityRecord<Image>) -> Self {
        let common = entity_common(&record.entity);
        ImageEntityRecordDto {
            common,
            location: record.entity.location.into(),
            r#override: record.r#override,
        }
    }
}

impl From<ImageLocation> for ImageLocationDto {
    fn from(loc: ImageLocation) -> Self {
        let bb = loc.bounding_box;
        ImageLocationDto {
            x: bb.min.x,
            y: bb.min.y,
            width: bb.width(),
            height: bb.height(),
            page: loc.page,
        }
    }
}

impl From<EntityRecord<Audio>> for AudioEntityRecordDto {
    fn from(record: EntityRecord<Audio>) -> Self {
        let common = entity_common(&record.entity);
        AudioEntityRecordDto {
            common,
            location: record.entity.location.into(),
            r#override: record.r#override,
        }
    }
}

impl From<AudioLocation> for AudioLocationDto {
    fn from(loc: AudioLocation) -> Self {
        AudioLocationDto {
            start_ms: loc.span.start_millis(),
            end_ms: loc.span.end_millis(),
            speaker: loc.speaker_id.map(|s| s.as_str().to_owned()),
        }
    }
}
