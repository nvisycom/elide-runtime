//! Detection confidence as a validated numeric primitive.
//!
//! [`Confidence`] is a value in `[0.0, 1.0]` (NaN / ±∞ rejected).
//! [`ConfidenceThreshold`] is a lower bound on `Confidence`, with a
//! single named operation [`admits`] so threshold semantics don't
//! drift across consumer sites.
//!
//! [`admits`]: ConfidenceThreshold::admits

mod threshold;
mod value;

pub use self::threshold::ConfidenceThreshold;
pub use self::value::Confidence;
