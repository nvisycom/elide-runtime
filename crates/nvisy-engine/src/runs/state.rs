//! Persisted run state: the [`Run`] header + per-document
//! [`RunDocument`] bodies.
//!
//! Two-layer persistence in fjall:
//!
//! - [`Run`] (a metadata header) lives in the `run_headers`
//!   keyspace under [`CompositeKey(actor_id, run_id)`].
//! - [`RunDocument`] (one per input doc) lives in the `run_docs`
//!   keyspace under [`TripleKey(actor_id, run_id, doc_id)`]. Its
//!   [`body`](RunDocument::body) carries the recognized entities +
//!   reviewer overrides.
//!
//! Bytes (input + redacted output) are **not** persisted in run
//! keyspaces. Inputs live in the [`crate::FileRegistry`] before
//! the run starts; redacted outputs land back in the same files
//! keyspace via [`FileRegistry::put_file`] stamped with a
//! [`FileLineage::RedactedFrom`] so they're traceable back to
//! the run + source file. A [`RunDocument`] tracks both id sides
//! via [`input_file_id`](RunDocument::input_file_id) +
//! [`output_file_id`](RunDocument::output_file_id).
//!
//! [`CompositeKey(actor_id, run_id)`]: crate::registry::CompositeKey
//! [`TripleKey(actor_id, run_id, doc_id)`]: crate::registry::TripleKey
//! [`FileRegistry`]: crate::FileRegistry
//! [`FileRegistry::put_file`]: crate::FileRegistry::put_file
//! [`FileLineage::RedactedFrom`]: nvisy_core::FileLineage::RedactedFrom

use std::collections::HashMap;

use elide_core::entity::Entity;
use elide_core::modality::Modality;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use hipstr::HipStr;
use jiff::Timestamp;
use nvisy_core::plan::AnalyzerParams;
use nvisy_core::policy::RuleAction;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Top-level state of one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunState {
    /// Analyze phase is in flight (or queued).
    Analyzing,
    /// All per-doc analyses finished; reports are ready for review
    /// and override.
    AwaitingReview,
    /// Apply ran; every document succeeded.
    Applied,
    /// Apply ran; some documents succeeded, others failed. Per-doc
    /// [`RunDocState`] carries the detail.
    PartiallyApplied,
    /// The whole run failed before any per-doc work completed.
    Failed {
        /// Human-readable reason (e.g. "couldn't load policy X").
        reason: String,
    },
}

/// State of one document inside a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunDocState {
    /// Awaiting its turn on the analyze semaphore.
    Queued,
    /// Analyze is running.
    Analyzing,
    /// Analyze succeeded; entities + overrides recorded; awaiting
    /// reviewer / apply.
    AwaitingReview,
    /// Apply ran; redacted bytes + audit recorded.
    Applied,
    /// Analyze or apply errored; reason in `reason`.
    Failed {
        /// Human-readable failure reason.
        reason: String,
    },
    /// Per-doc timeout fired before completion.
    TimedOut,
}

/// Run header — short metadata blob persisted under
/// `(actor_id, run_id)`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// Stable identifier. Engine-minted UUIDv7 at start time so
    /// the value carries its own creation order.
    pub id: Uuid,
    /// Run state.
    pub state: RunState,
    /// UUIDv7 timestamp the run was started.
    #[schemars(with = "String")]
    pub started_at: Timestamp,
    /// UUIDv7 timestamp of the most recent state transition.
    #[schemars(with = "String")]
    pub updated_at: Timestamp,
    /// Policies the caller submitted, as resource refs. Loaded
    /// from [`crate::keyspace::policy`] at start time; stable for the
    /// lifetime of the run.
    pub policy_refs: Vec<ResourceRef>,
    /// Contexts the caller submitted, as resource refs.
    pub context_refs: Vec<ResourceRef>,
    /// Per-request metadata the caller attached (merged with each
    /// document's descriptor at policy-evaluation time).
    pub metadata: HashMap<String, String>,
    /// One entry per input document; the body lives in `run_docs`
    /// under a `(actor_id, run_id, doc_id)` triple key.
    pub document_ids: Vec<Uuid>,
    /// Recognition plan the caller supplied — engine compiles it
    /// per modality at analyze time. Persisted on the header so
    /// apply (run separately, potentially after a process restart)
    /// can re-resolve the same recognition shape.
    pub analyzer: AnalyzerParams,
    /// Cap on how many per-doc analyses run concurrently. Honoured
    /// by both analyze and apply.
    pub concurrency: usize,
}

/// Reference to a stored policy / context resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    /// Resource UUID.
    pub id: Uuid,
    /// Resource version. Two `(id, version)` pairs reference
    /// distinct stored blobs.
    pub version: Version,
}

/// One document inside a run — the per-doc body persisted under
/// `(actor_id, run_id, doc_id)`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunDocument {
    /// Engine-minted UUIDv7 at start time.
    pub id: Uuid,
    /// Id of the input file this doc analyses. Persisted on the
    /// row so apply (run separately, potentially after a process
    /// restart) can re-resolve the bytes via
    /// [`FileRegistry::get_file_bytes`] without re-deriving from
    /// the run header's `document_ids`.
    ///
    /// [`FileRegistry::get_file_bytes`]: crate::FileRegistry::get_file_bytes
    pub input_file_id: Uuid,
    /// Id of the redacted output file, when apply succeeded for
    /// this doc. `None` pre-apply and on per-doc failure. Lookup
    /// via [`FileRegistry::get_file_bytes`]; the metadata blob's
    /// [`lineage`](nvisy_core::FileMetadata::lineage) field
    /// points back at this run.
    ///
    /// [`FileRegistry::get_file_bytes`]: crate::FileRegistry::get_file_bytes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_file_id: Option<Uuid>,
    /// State of the per-doc lifecycle.
    pub state: RunDocState,
    /// The modality elide's codec resolved this doc to; pins the
    /// active variant of [`body`](Self::body).
    pub modality: ModalityKind,
    /// Per-modality recognized entities + reviewer overrides.
    pub body: DocBody,
}

/// Discriminator for the modality the codec resolved a document
/// to. Pinned at decode time inside [`RunDocument::modality`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModalityKind {
    /// `Text` modality.
    Text,
    /// `Tabular` modality.
    Tabular,
    /// `Image` modality.
    Image,
    /// `Audio` modality.
    Audio,
}

/// Per-modality body: the entities recognized in this doc plus
/// any reviewer overrides. Tagged by `modality` so the read side
/// can pick the right variant from the JSON.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum DocBody {
    /// Text entities.
    Text {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Text>>,
    },
    /// Tabular entities.
    Tabular {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Tabular>>,
    },
    /// Image entities.
    Image {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Image>>,
    },
    /// Audio entities.
    Audio {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Audio>>,
    },
}

impl DocBody {
    /// Empty body for the given modality. Engine constructs this
    /// before recognition runs so the doc state is queryable from
    /// the moment the run starts.
    pub fn empty(modality: ModalityKind) -> Self {
        match modality {
            ModalityKind::Text => Self::Text {
                entities: Vec::new(),
            },
            ModalityKind::Tabular => Self::Tabular {
                entities: Vec::new(),
            },
            ModalityKind::Image => Self::Image {
                entities: Vec::new(),
            },
            ModalityKind::Audio => Self::Audio {
                entities: Vec::new(),
            },
        }
    }
}

/// One recognized entity plus the optional reviewer override.
///
/// The bound mirrors elide's [`Entity<M>`]: serialization needs
/// `M::Location` and `M::Data` (de)serializable, and JsonSchema
/// derivation needs them schema-able. All four modalities elide
/// ships satisfy these under the `serde` + `schema` features.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "M::Location: Serialize + for<'a> Deserialize<'a>, \
                  M::Data: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M::Location: JsonSchema, M::Data: JsonSchema")]
pub struct EntityRecord<M: Modality> {
    /// The elide entity, as recognition produced it.
    pub entity: Entity<M>,
    /// Reviewer-supplied override. `None` means "use the policy's
    /// decision"; `Some(action)` overrides it for this specific
    /// entity at apply time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#override: Option<RuleAction>,
}

impl<M: Modality> EntityRecord<M> {
    /// New record over `entity`, no override.
    pub fn new(entity: Entity<M>) -> Self {
        Self {
            entity,
            r#override: None,
        }
    }
}

/// Free-form reason carrier reused by `Failed`/`TimedOut` states
/// at the per-doc API surface, when callers want a typed handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureReason(pub HipStr<'static>);
