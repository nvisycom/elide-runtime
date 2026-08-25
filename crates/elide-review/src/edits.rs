//! [`EditSet`]: every reviewer edit for one document, by modality.
//!
//! A list per modality rather than a map keyed by entity id: edits
//! feed independent channels, so one entity can legitimately carry
//! both a retag and an operator override. A map would hold one and
//! silently drop the other.
//!
//! Ordered, so a contradiction is reported against the edit that
//! caused it rather than against whichever the hash landed on last.

use std::collections::HashMap;

use elide::Result;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide_governance::modality::RedactableModality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::edit::{Channel, Edit};

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

    /// Reject contradictory edits, in every modality.
    ///
    /// Two edits on one entity conflict when they answer the same
    /// question differently: two operators, a suppress against a
    /// redact, two labels. Edits on different channels compose, and
    /// so do two that merge cleanly — a retag setting the label
    /// beside one setting the location, or a repeated suppress.
    ///
    /// Call before [`apply`](Self::apply), so a self-contradicting
    /// payload fails at the boundary rather than producing output
    /// that honoured half of it. `elide-pipeline` runs this at the
    /// start of every anonymize.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](elide::ErrorKind::Configuration) naming
    /// the entity and both edits.
    ///
    pub fn validate(&self) -> Result<()> {
        validate_modality(&self.text, "text")?;
        validate_modality(&self.tabular, "tabular")?;
        validate_modality(&self.image, "image")?;
        validate_modality(&self.audio, "audio")
    }
}

/// Reject contradictions among one modality's edits.
fn validate_modality<M: RedactableModality>(edits: &[Edit<M>], modality: &str) -> Result<()> {
    // Per entity, the edit already seen on each channel. An `Add`
    // names no entity, so it can never contradict anything.
    let mut seen: HashMap<(Uuid, Channel), &Edit<M>> = HashMap::new();

    for edit in edits {
        let Some(id) = edit.target() else {
            continue;
        };
        let channel = edit.channel();
        match seen.get(&(id, channel)) {
            Some(earlier) if !earlier.merges_with(edit) => {
                return Err(earlier.conflict_with(edit, modality, id));
            }
            // Merges cleanly, or first on this channel.
            _ => {
                seen.insert((id, channel), edit);
            }
        }
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
