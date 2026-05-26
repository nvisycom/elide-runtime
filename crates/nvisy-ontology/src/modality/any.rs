//! [`AnyModality`] — sealed enum over the four built-in modalities.
//!
//! Used **only** by provenance types ([`Audit`], [`AuditEntry`],
//! [`RedactionMap`]) where a single compliance artifact for one
//! `ContentSource` must collect entities across every modality that
//! processed the source. Everything else in the pipeline stays
//! generic over [`Modality`].
//!
//! [`Audit`]: crate::provenance::Audit
//! [`AuditEntry`]: crate::provenance::AuditEntry
//! [`RedactionMap`]: crate::provenance::RedactionMap

use std::fmt;

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Audio, Image, Modality, Overlap, Tabular, Text};

impl Default for AnyModality {
    /// Defaults to an empty [`Text`] location — value-only
    /// annotations have no concrete coordinates anywhere, but text is
    /// the common case.
    fn default() -> Self {
        Self::Text(Text::default())
    }
}

/// Type-erased view over a per-modality location.
///
/// Use [`From`] to lift a concrete [`Text`] / [`Image`] / [`Audio`] /
/// [`Tabular`] into [`AnyModality`] at the provenance boundary.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AnyModality {
    /// Text-modality coordinates.
    Text(Text),
    /// Image-modality coordinates.
    Image(Image),
    /// Audio-modality coordinates.
    Audio(Audio),
    /// Tabular-modality coordinates.
    Tabular(Tabular),
}

impl AnyModality {
    /// If this is a text location, return a reference to it.
    pub fn as_text(&self) -> Option<&Text> {
        match self {
            Self::Text(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is an image location, return a reference to it.
    pub fn as_image(&self) -> Option<&Image> {
        match self {
            Self::Image(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is an audio location, return a reference to it.
    pub fn as_audio(&self) -> Option<&Audio> {
        match self {
            Self::Audio(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is a tabular location, return a reference to it.
    pub fn as_tabular(&self) -> Option<&Tabular> {
        match self {
            Self::Tabular(loc) => Some(loc),
            _ => None,
        }
    }
}

/// Diagnostic display format for logging; not intended for round-trip
/// parsing.
impl fmt::Display for AnyModality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(loc) => write!(f, "text:{}..{}", loc.start_offset, loc.end_offset),
            Self::Image(loc) => {
                let bb = &loc.bounding_box;
                write!(
                    f,
                    "image:{:.0},{:.0} {:.0}x{:.0}",
                    bb.x, bb.y, bb.width, bb.height
                )
            }
            Self::Audio(loc) => {
                let ts = &loc.time_span;
                write!(f, "audio:{:.2}s..{:.2}s", ts.start_secs(), ts.end_secs())
            }
            Self::Tabular(loc) => {
                write!(f, "tabular:r{}c{}", loc.row_index, loc.column_index)
            }
        }
    }
}

/// [`AnyModality`] is used only as the location type on
/// `Entity<AnyModality>` inside provenance — never as the parameter on
/// `Document`/`Block`. The block/artefact/doc-meta associated types
/// are therefore `()`.
impl Modality for AnyModality {
    type BlockKind = ();
    type Artefact = ();
    type Metadata = ();
}

impl Overlap for AnyModality {
    fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a.overlaps(b),
            (Self::Image(a), Self::Image(b)) => a.overlaps(b),
            (Self::Audio(a), Self::Audio(b)) => a.overlaps(b),
            (Self::Tabular(a), Self::Tabular(b)) => a.overlaps(b),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::{BoundingBox, TimeSpan};

    #[test]
    fn cross_modality_no_overlap() {
        let text = AnyModality::Text(Text::new(0, 10));
        let image = AnyModality::Image(Image::new(BoundingBox {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }));
        let audio = AnyModality::Audio(Audio::new(TimeSpan::new(0, 1_000_000)));
        let tabular = AnyModality::Tabular(Tabular::new(0, 0));

        assert!(!text.overlaps(&image));
        assert!(!image.overlaps(&audio));
        assert!(!audio.overlaps(&tabular));
    }
}
