//! Per-request, per-modality region annotations.
//!
//! Groups elide's modality-typed [`Annotations<M>`] into one
//! wire type per modality so the analyzer can hand each modality
//! its own bag at compile time.
//!
//! [`Annotations<M>`]: elide_core::recognition::annotation::Annotations

use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use elide_core::recognition::annotation::Annotations;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Caller-supplied per-modality region annotations for one
/// analysis.
///
/// Modality bag: each field carries the caller's inclusions and
/// exclusions for that modality. Slots are attached to the
/// analyzer of their modality at compile time; a document that
/// decodes to text pulls its annotations from [`text`], a
/// document that decodes to an image pulls from [`image`], etc.
///
/// Empty by default: the common case is no per-request
/// annotations.
///
/// [`text`]: AnyAnnotations::text
/// [`image`]: AnyAnnotations::image
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct AnyAnnotations {
    /// Text-modality region annotations.
    pub text: Annotations<Text>,
    /// Tabular-modality region annotations.
    pub tabular: Annotations<Tabular>,
    /// Image-modality region annotations.
    pub image: Annotations<Image>,
    /// Audio-modality region annotations.
    pub audio: Annotations<Audio>,
}

impl Default for AnyAnnotations {
    fn default() -> Self {
        Self {
            text: Annotations::new(),
            tabular: Annotations::new(),
            image: Annotations::new(),
            audio: Annotations::new(),
        }
    }
}
