//! Primitive value types wire schemas carry directly.
//!
//! Bounding boxes, confidence, colors, languages, country codes,
//! time spans, and friends. Wire types on [`plan`] and [`policy`]
//! use these as leaf fields.
//!
//! [`plan`]: crate::plan
//! [`policy`]: crate::policy

pub use elide_core::primitive::{
    BoundingBox, Color, Confidence, ConfidenceThreshold, CountryCode, Dimensions, Dpi, Language,
    LanguageProvenance, LanguageSpan, LanguageTag, Languages, OcrMode, PixelRegion, Point, Polygon,
    TimeSpan, UnitBoundingBox,
};
