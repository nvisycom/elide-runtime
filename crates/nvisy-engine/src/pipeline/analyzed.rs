//! Analyze → anonymize bridge: what
//! [`Engine::analyze_document`] returns and what
//! [`Engine::anonymize_document`] accepts.
//!
//! [`AnalyzedDocument`] mirrors elide's [`Report`] shape: a
//! body group + zero-or-more container part groups (DOCX
//! embedded images, archive members, ...) keyed by
//! container-private part id. Every group is a
//! [`RecognizedGroup`] tagged by modality so the serialized
//! form round-trips cleanly. Reviewer overrides live per-entity
//! inside [`EntityRecord`].
//!
//! Hosts hold this value between analyze and anonymize and may
//! persist it however they like (`serde` derives are on
//! everything).
//!
//! [`Engine::analyze_document`]: super::Engine::analyze_document
//! [`Engine::anonymize_document`]: super::Engine::anonymize_document
//!
//! [`Report`]: elide::Report

use std::collections::HashMap;

use elide::recognition::Scope;
use elide_core::entity::Entity;
use elide_core::modality::Modality;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_schema::policy::PolicyAction;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What detection found in one document: the body group plus
/// per-container-part groups (each tagged by modality) plus a
/// snapshot of the recognition [`Scope`] the entities were
/// scored against.
///
/// The scope snapshot travels with the entities so anonymize
/// can rebuild an orchestrator against exactly the vocabulary
/// analyze used. Anything a policy predicate compares against
/// (label catalog, document-level classification labels,
/// asserted languages / jurisdictions) is here.
///
/// `correlation_id` on the persisted scope is always `None`; the
/// anonymize call supplies a fresh id from the passed
/// [`Document`](nvisy_schema::file::Document) so anonymize-side
/// tracing spans are distinct from the analyze-side ones.
///
/// [`Scope`]: elide::recognition::Scope
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzedDocument {
    /// The body group. `None` when no body pipeline produced
    /// entities (pre-analyze, or the codec resolved the doc to a
    /// modality with no pipeline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<RecognizedGroup>,
    /// One entry per container part the orchestrator surfaced.
    /// Keyed by the container-private part id (e.g. a DOCX zip
    /// entry name like `"word/media/image1.png"`); each value
    /// carries that part's modality + entities.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub parts: HashMap<String, RecognizedGroup>,
    /// Recognition scope snapshot: the resolved label catalog +
    /// asserted languages, countries, and document labels. Held
    /// so [`Engine::anonymize_document`] can compile against the
    /// same vocabulary analyze used without the caller
    /// re-passing an `AnalyzerParams`.
    ///
    /// [`Engine::anonymize_document`]: super::Engine::anonymize_document
    #[serde(default)]
    pub scope: Scope,
}

/// A modality-tagged group of recognized entities. The unit
/// [`AnalyzedDocument`] stores in `body` and in every `parts` entry.
///
/// Tagged by `modality` (snake_case) so deserialization picks the
/// right variant and the entity vec inside is statically typed
/// per modality — apply-time we hand each variant back to elide
/// as a `Vec<Entity<M>>` for the appropriate `M`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum RecognizedGroup {
    /// Text entities.
    Text {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Text>>,
    },
    /// Tabular entities.
    #[cfg(feature = "internal_tabular")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tabular")))]
    Tabular {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Tabular>>,
    },
    /// Image entities.
    #[cfg(feature = "internal_image")]
    #[cfg_attr(docsrs, doc(cfg(feature = "image")))]
    Image {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Image>>,
    },
    /// Audio entities.
    #[cfg(feature = "internal_audio")]
    #[cfg_attr(docsrs, doc(cfg(feature = "audio")))]
    Audio {
        /// Recognized entities, in source-coordinate order.
        entities: Vec<EntityRecord<Audio>>,
    },
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
    pub r#override: Option<PolicyAction>,
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
