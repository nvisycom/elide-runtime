//! Analyze → apply bridge: what `analyze()` returns and what
//! `apply()` accepts.
//!
//! [`DocBody`] mirrors elide's [`Report`] shape: a body group +
//! zero-or-more container part groups (DOCX embedded images,
//! archive members, ...) keyed by container-private part id.
//! Every group is a [`RecognizedGroup`] tagged by modality so the
//! serialized form round-trips cleanly. Reviewer overrides live
//! per-entity inside [`EntityRecord`].
//!
//! Hosts hold this value between `analyze()` and `apply()` and
//! may persist it however they like (`serde` derives are on
//! everything).
//!
//! [`Report`]: elide::Report

use std::collections::HashMap;

use elide_core::entity::Entity;
use elide_core::modality::Modality;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_schema::policy::RuleAction;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The body of one document as detection saw it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocBody {
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
}

/// A modality-tagged group of recognized entities. The unit
/// [`DocBody`] stores in `body` and in every `parts` entry.
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
