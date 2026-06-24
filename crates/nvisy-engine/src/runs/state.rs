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
//! - The post-apply redacted bytes live in a third keyspace
//!   (`run_artifacts`) so the body stays cheap to load for review
//!   surfaces.
//!
//! [`CompositeKey(actor_id, run_id)`]: crate::registry::CompositeKey
//! [`TripleKey(actor_id, run_id, doc_id)`]: crate::registry::TripleKey

use std::collections::HashMap;

use elide_core::entity::Entity;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use hipstr::HipStr;
use jiff::Timestamp;
use nvisy_core::plan::AnalyzerSpec;
use nvisy_core::policy::RuleAction;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Top-level state of one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// Stable identifier. Engine-minted UUIDv7 at start time so
    /// the value carries its own creation order.
    pub id: Uuid,
    /// Run state.
    pub state: RunState,
    /// UUIDv7 timestamp the run was started.
    pub started_at: Timestamp,
    /// UUIDv7 timestamp of the most recent state transition.
    pub updated_at: Timestamp,
    /// Policies the caller submitted, as resource refs. Loaded
    /// from [`crate::policies`] at start time; stable for the
    /// lifetime of the run.
    pub policy_refs: Vec<ResourceRef>,
    /// Contexts the caller submitted, as resource refs.
    pub context_refs: Vec<ResourceRef>,
    /// Per-request metadata the caller attached (merged with each
    /// document's descriptor at policy-evaluation time).
    pub metadata: HashMap<String, String>,
    /// One entry per input document; the body lives in `run_docs`
    /// under [`TripleKey(actor_id, run_id, doc_id)`].
    ///
    /// [`TripleKey(actor_id, run_id, doc_id)`]: crate::registry::TripleKey
    pub document_ids: Vec<Uuid>,
    /// Recognition plan the caller supplied — engine compiles it
    /// per modality at analyze time. Persisted on the header so
    /// apply (run separately, potentially after a process restart)
    /// can re-resolve the same recognition shape.
    pub analyzer: AnalyzerSpec,
    /// Cap on how many per-doc analyses run concurrently. Honoured
    /// by both analyze and apply.
    pub concurrency: usize,
}

/// Reference to a stored policy / context resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDocument {
    /// Engine-minted UUIDv7 at start time.
    pub id: Uuid,
    /// File extension the codec registry resolves on (set from
    /// [`DocumentInput::extension`] at start time).
    ///
    /// [`DocumentInput::extension`]: super::input::DocumentInput::extension
    pub extension: String,
    /// Caller-supplied descriptor labels (drive
    /// [`DocumentPredicate::HasLabel`] gating).
    ///
    /// [`DocumentPredicate::HasLabel`]: nvisy_core::policy::DocumentPredicate::HasLabel
    pub descriptor_labels: Vec<String>,
    /// Caller-supplied descriptor metadata (drive
    /// [`DocumentPredicate::HasMetadata`] gating).
    ///
    /// [`DocumentPredicate::HasMetadata`]: nvisy_core::policy::DocumentPredicate::HasMetadata
    pub descriptor_metadata: HashMap<String, String>,
    /// State of the per-doc lifecycle.
    pub state: RunDocState,
    /// The modality elide's codec resolved this doc to; pins the
    /// active variant of [`body`](Self::body).
    pub modality: ModalityKind,
    /// Per-modality recognized entities + reviewer overrides.
    pub body: DocBody,
    /// `true` when the post-apply redacted bytes for this doc
    /// exist under `(actor_id, run_id, doc_id)` in the
    /// `run_artifacts` keyspace.
    #[serde(default)]
    pub has_artifact: bool,
}

/// Discriminator for the modality the codec resolved a document
/// to. Pinned at decode time inside [`RunDocument::modality`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            ModalityKind::Text => Self::Text { entities: Vec::new() },
            ModalityKind::Tabular => Self::Tabular { entities: Vec::new() },
            ModalityKind::Image => Self::Image { entities: Vec::new() },
            ModalityKind::Audio => Self::Audio { entities: Vec::new() },
        }
    }
}

/// One recognized entity plus the optional reviewer override.
///
/// The serde bound mirrors elide's [`Entity<M>`]: serialization
/// needs `M::Location` and `M::Data` (de)serializable, which all
/// four modalities elide ships satisfy under the `serde` feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "M::Location: Serialize + for<'a> Deserialize<'a>, \
                  M::Data: Serialize + for<'a> Deserialize<'a>")]
pub struct EntityRecord<M: elide_core::modality::Modality> {
    /// The elide entity, as recognition produced it.
    pub entity: Entity<M>,
    /// Reviewer-supplied override. `None` means "use the policy's
    /// decision"; `Some(action)` overrides it for this specific
    /// entity at apply time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#override: Option<RuleAction>,
}

impl<M: elide_core::modality::Modality> EntityRecord<M> {
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
