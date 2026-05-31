//! Audio redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modality::{LeakProfile, RedactionStrategy};

/// Audio redaction strategy.
///
/// The [`Default`] impl returns [`Silence`] — preserves duration while
/// removing the audible content.
///
/// [`Silence`]: AudioStrategy::Silence
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioStrategy {
    /// Replace with silence.
    #[default]
    Silence,
    /// Remove the segment entirely.
    Remove,
}

/// Parameter-less tag for each [`AudioStrategy`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudioMethodTag {
    /// Tag for [`AudioStrategy::Silence`].
    Silence,
    /// Tag for [`AudioStrategy::Remove`].
    Remove,
}

impl RedactionStrategy for AudioStrategy {
    /// - [`Silence`] is [`Partial`] — a silence of known duration on
    ///   the timeline is observable.
    /// - [`Remove`] is [`Irrecoverable`] — the segment is cut, the
    ///   timeline shifts, no trace remains.
    ///
    /// [`Silence`]: Self::Silence
    /// [`Remove`]: Self::Remove
    /// [`Partial`]: LeakProfile::Partial
    /// [`Irrecoverable`]: LeakProfile::Irrecoverable
    fn leak_profile(&self) -> LeakProfile {
        match self {
            Self::Silence => LeakProfile::Partial,
            Self::Remove => LeakProfile::Irrecoverable,
        }
    }
}
