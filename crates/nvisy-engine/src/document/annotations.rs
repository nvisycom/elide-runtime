//! Per-content user-supplied annotations, bucketed per modality.

use nvisy_core::entity::{Annotation, LabelAnnotation};
use nvisy_core::modality::{Audio, Image, Tabular, Text};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-modality buckets of user-supplied annotations on a piece of
/// content.
///
/// Each modality-typed [`Annotation<M>`] targets a `Document<M>`
/// envelope of the same modality; document-level
/// [`LabelAnnotation`]s apply to every envelope spawned from the
/// source.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnyAnnotations {
    /// Annotations targeting text-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<Annotation<Text>>,
    /// Annotations targeting tabular-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabular: Vec<Annotation<Tabular>>,
    /// Annotations targeting image-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<Annotation<Image>>,
    /// Annotations targeting audio-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio: Vec<Annotation<Audio>>,
    /// Document-level classification labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<LabelAnnotation>,
}

impl AnyAnnotations {
    /// `true` when every bucket is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.tabular.is_empty()
            && self.image.is_empty()
            && self.audio.is_empty()
            && self.labels.is_empty()
    }
}
