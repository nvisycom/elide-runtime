//! Primitive value types re-exported from `elide_core::primitive`.
//!
//! Wire types on [`plan`], [`policy`], and [`context`] carry
//! these directly. The re-export means an SDK caller can
//! construct them without adding `elide-core` as a separate
//! dep.
//!
//! [`plan`]: crate::plan
//! [`policy`]: crate::policy
//! [`context`]: crate::context

pub use elide_core::primitive::{
    BoundingBox, Color, ConfidenceThreshold, CountryCode, LanguageTag, Languages, Point, Polygon,
    TimeSpan,
};
