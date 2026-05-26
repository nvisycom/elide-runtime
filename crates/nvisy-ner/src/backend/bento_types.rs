//! Wire types for the externalised `inference-gliner` Bento.
//!
//! Mirror of `nvisy_core.ner.v1` from [`nvisycom/inference`].
//! Schema version v1. Field names are camelCase on the wire to match
//! the Python service.
//!
//! Kept in a separate module so the transport details of
//! [`BentoNerBackend`] stay isolated from the trait implementation
//! and easy to grep when the upstream schema changes.
//!
//! [`BentoNerBackend`]: super::BentoNerBackend
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference

use nvisy_ontology::entity::{Entity, EntityKind, ModelKind, RecognitionMethod};
use nvisy_ontology::modality::Text;
use nvisy_ontology::primitive::Confidence;
use serde::{Deserialize, Serialize};

/// Outer batch wrapper our `inference-gliner` API expects.
///
/// Single- and multi-text calls share the same shape — a single
/// recognition is just a batch of one. The service responds with
/// `Vec<WireResponse>` in the same order as `requests`.
#[derive(Serialize)]
pub(super) struct WireBatch {
    /// One recognition request per text. The response array on the
    /// wire has the same length and the same ordering.
    pub requests: Vec<WireRequest>,
}

/// One recognition request: a text plus the per-call inference
/// knobs the GLiNER model exposes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireRequest {
    /// The text to recognise entities in. Byte offsets in the
    /// response refer back into this string.
    pub text: String,
    /// Entity kinds the caller is interested in. GLiNER is
    /// zero-shot, so this list directly shapes inference — sending
    /// an empty list is meaningless (the service has nothing to
    /// look for) and the runtime short-circuits to an empty result
    /// without making the call.
    pub kinds: Vec<EntityKind>,
    /// Lower bound on per-entity score. Entities below this
    /// threshold are dropped server-side. The runtime keeps this at
    /// `0.0` and post-filters locally so threshold decisions stay
    /// in one place.
    pub threshold: f64,
    /// Optional BCP-47 language hint forwarded from
    /// [`NerParams::language`]. Multilingual GLiNER variants may
    /// ignore it; monolingual variants may validate it. Omitted on
    /// the wire when `None`.
    ///
    /// [`NerParams::language`]: super::NerParams::language
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// One per-request recognition result.
///
/// The vec the service returns has one `WireResponse` per
/// [`WireRequest`] in the original [`WireBatch`], in the same
/// order.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireResponse {
    /// Identity of the underlying model that produced these
    /// entities — surfaced into [`RecognitionMethod::nlp_ner`] so
    /// audit trails attribute correctly. Echoed per response so a
    /// service hosting multiple models can report different
    /// identities within one batch.
    ///
    /// [`RecognitionMethod::nlp_ner`]: nvisy_ontology::entity::RecognitionMethod::nlp_ner
    pub model: String,
    /// Recognised entities, already classified into the canonical
    /// [`EntityKind`] taxonomy by the service. Defaults to empty
    /// when the service omits the field (e.g. nothing was found).
    #[serde(default)]
    pub entities: Vec<WireEntity>,
}

/// One recognised entity span on the wire.
///
/// Translated into an ontology [`Entity`] by `wire_to_entity` —
/// the byte offsets become the entity's text span, `score` becomes
/// a clamped [`Confidence`], and the surrounding response's
/// `model` is stamped onto the recognition method.
///
/// [`Entity`]: nvisy_ontology::entity::Entity
/// [`Confidence`]: nvisy_ontology::primitive::Confidence
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireEntity {
    /// Canonical entity kind the service classified this span as.
    /// Label-map translation happens server-side so swapping the
    /// underlying model never changes runtime code.
    pub kind: EntityKind,
    /// Raw model score in `[0.0, 1.0]`. Clamped to the
    /// [`Confidence`] range when converted; the runtime keeps the
    /// raw value rather than re-thresholding.
    ///
    /// [`Confidence`]: nvisy_ontology::primitive::Confidence
    pub score: f64,
    /// Byte offset of the entity's start within
    /// [`WireRequest::text`].
    pub start: usize,
    /// Byte offset one past the entity's end within
    /// [`WireRequest::text`] (half-open `start..end`).
    pub end: usize,
}

impl WireEntity {
    /// Convert this wire entity into an ontology [`Entity`],
    /// stamping the model identity reported by the inference
    /// service on the [`RecognitionMethod`].
    pub fn to_entity(&self, model: &str) -> Entity<Text> {
        Entity::builder()
            .with_category(self.kind.category())
            .with_entity_kind(self.kind)
            .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                model.to_owned(),
                ModelKind::SelfHosted,
            )])
            .with_confidence(Confidence::clamped(self.score))
            .with_location(Text::new(self.start, self.end))
            .build()
            .expect("required fields provided")
    }
}
