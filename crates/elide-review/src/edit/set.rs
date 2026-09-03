//! [`EditSet`]: every reviewer edit for one document, by modality.
//!
//! A list per modality rather than a map keyed by entity id: edits
//! feed independent channels, so one entity can legitimately carry
//! both a retag and a suppression. A map would hold one and
//! silently drop the other.
//!
//! Ordered, so a contradiction is reported against the edit that
//! caused it rather than against whichever the hash landed on last.

use std::collections::HashMap;

use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide::{PartId, Report};
use elide_governance::modality::RedactableModality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::EditError;
use super::record::{Channel, Edit};

/// Reviewer edits for one document, one list per modality.
///
/// Empty by default: an audit nobody has reviewed carries none.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct EditSet {
    /// Edits to text entities.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<Edit<Text>>,
    /// Edits to tabular entities.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tabular: Vec<Edit<Tabular>>,
    /// Edits to image entities.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<Edit<Image>>,
    /// Edits to audio entities.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub audio: Vec<Edit<Audio>>,
}

impl EditSet {
    /// Record an edit.
    ///
    /// Appends rather than replaces: edits feed independent
    /// channels, so retagging an entity and suppressing it are both
    /// legitimate at once. Two that answer the same question
    /// differently are rejected by [`validate`](Self::validate).
    ///
    /// The modality is the entity's own, so an edit carrying a
    /// location carries that modality's — a text entity cannot be
    /// given an image span, and the mismatch will not compile.
    pub fn edit<M: EditBucket>(&mut self, edit: Edit<M>) -> &mut Self {
        M::bucket_mut(self).push(edit);
        self
    }

    /// Every edit recorded for the entity `id`, in order.
    #[must_use]
    pub fn edits_for<M: EditBucket>(&self, id: Uuid) -> Vec<&Edit<M>> {
        M::bucket(self)
            .iter()
            .filter(|edit| edit.target() == Some(id))
            .collect()
    }

    /// Drop every edit recorded for the entity `id`.
    ///
    /// Returns how many were dropped.
    pub fn unedit<M: EditBucket>(&mut self, id: Uuid) -> usize {
        let bucket = M::bucket_mut(self);
        let before = bucket.len();
        bucket.retain(|edit| edit.target() != Some(id));
        before - bucket.len()
    }

    /// Whether no modality carries an edit.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.tabular.is_empty()
            && self.image.is_empty()
            && self.audio.is_empty()
    }

    /// How many edits this set carries across every modality.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len() + self.tabular.len() + self.image.len() + self.audio.len()
    }

    /// Reject an edit set this report cannot take.
    ///
    /// Two things go wrong, and both are silent without this. Two
    /// edits can answer the same question about one entity
    /// differently — two labels for one retag — and preferring
    /// either would discard a decision a reviewer made. Or an edit
    /// can name an entity the report does not hold, which
    /// [`apply`](Self::apply) skips without a word, so a reviewer
    /// would be told their decision took effect when the document
    /// says otherwise.
    ///
    /// Call before [`apply`](Self::apply), which is infallible and
    /// applies what it is given.
    ///
    /// Edits on different channels still compose, and composable
    /// pairs still merge: a retag beside a suppression is two
    /// legitimate decisions, two retags setting disjoint fields are
    /// one correction split in two, and a repeated suppress is a
    /// duplicate rather than a conflict.
    ///
    /// # Errors
    ///
    /// Returns the first [`EditError`] found. The set is unchanged
    /// either way — nothing is applied until [`apply`](Self::apply).
    pub fn validate(&self, report: &Report) -> Result<(), EditError> {
        validate_modality::<Text>(&self.text, report)?;
        validate_modality::<Tabular>(&self.tabular, report)?;
        validate_modality::<Image>(&self.image, report)?;
        validate_modality::<Audio>(&self.audio, report)
    }
}

/// Reject contradictions and unknown targets among one modality's
/// edits.
///
/// Tracks every edit already seen on a channel, not just the last:
/// a retag setting the label, then one setting the location, then a
/// second setting the label is a contradiction with the *first*,
/// and comparing only against the most recent would miss it.
fn validate_modality<M: RedactableModality>(
    edits: &[Edit<M>],
    report: &Report,
) -> Result<(), EditError> {
    // Per entity and channel, every edit seen so far. An `Add` names
    // no entity, so it can neither contradict anything nor go
    // missing from the report.
    let mut seen: HashMap<(Uuid, Channel), Vec<&Edit<M>>> = HashMap::new();

    for edit in edits {
        // An add names no entity — the engine mints the id when it
        // lands — but it addresses a part, and `include_part` is
        // silent about one the report does not carry. A named part
        // must exist; an unnamed one means the sole document, which
        // only resolves when there is exactly one.
        if let Edit::Add(add) = edit {
            match add.part.as_deref() {
                Some(part) => {
                    // Checked through `part_entities::<M>`, not by
                    // path alone: `include_part` returns `false` for
                    // a modality mismatch just as silently as for a
                    // missing part, and landing ignores that bool.
                    // A text add naming an image part would
                    // otherwise validate and then vanish.
                    let id = PartId::from_segments(part.to_vec());
                    if report.part_entities::<M>(&id).is_none() {
                        return Err(EditError::UnknownPart {
                            part: part.to_vec(),
                            modality: M::NAME,
                        });
                    }
                }
                None => {
                    // Zero and many fail for different reasons and
                    // take different fixes, so they are told apart:
                    // naming a part answers the second and cannot
                    // answer the first.
                    let documents = report.part_ids().filter(|(id, _)| id.depth() == 1).count();
                    match documents {
                        1 => {}
                        0 => return Err(EditError::EmptyReport),
                        _ => return Err(EditError::AmbiguousPart { documents }),
                    }
                }
            }
        }
        let Some(id) = edit.target() else {
            continue;
        };
        if report.entity_anywhere::<M>(id).is_none() {
            return Err(EditError::unknown_target::<M>(id));
        }
        let channel = seen.entry((id, edit.channel())).or_default();
        if let Some(earlier) = channel.iter().find(|e| !e.merges_with(edit)) {
            return Err(EditError::contradiction::<M>(
                id,
                earlier.name(),
                edit.name(),
            ));
        }
        channel.push(edit);
    }
    Ok(())
}

/// Reaching one modality's edits generically.
///
/// Typed calls are generic over the modality, and [`EditSet`]'s
/// four lists are plain fields, so this maps one to the other.
pub trait EditBucket: RedactableModality + Sized {
    /// This modality's edits, in the order they were recorded.
    fn bucket(set: &EditSet) -> &Vec<Edit<Self>>;

    /// The same list, for recording one.
    fn bucket_mut(set: &mut EditSet) -> &mut Vec<Edit<Self>>;
}

macro_rules! impl_edit_bucket {
    ($($modality:ty => $field:ident),+ $(,)?) => {
        $(
            impl EditBucket for $modality {
                fn bucket(set: &EditSet) -> &Vec<Edit<Self>> {
                    &set.$field
                }

                fn bucket_mut(set: &mut EditSet) -> &mut Vec<Edit<Self>> {
                    &mut set.$field
                }
            }
        )+
    };
}

impl_edit_bucket! {
    Text => text,
    Tabular => tabular,
    Image => image,
    Audio => audio,
}
