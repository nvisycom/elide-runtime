//! Primitive value types wire schemas carry directly.
//!
//! Bounding boxes, confidence, colors, languages, country codes,
//! time spans, and friends. Appear as leaf fields on entities,
//! on the recognition [`AuditContext`], on [`plan`] recognizer
//! params, and on [`policy`] operators.
//!
//! [`AuditContext`]: crate::AuditContext
//! [`plan`]: crate::plan
//! [`policy`]: crate::policy

pub use nvisy_schema::primitive::{
    BoundingBox, Color, Confidence, ConfidenceThreshold, CountryCode, Dimensions, Dpi, Language,
    LanguageProvenance, LanguageSpan, LanguageTag, Languages, PixelRegion, Point, Polygon,
    TimeSpan, UnitBoundingBox,
};
