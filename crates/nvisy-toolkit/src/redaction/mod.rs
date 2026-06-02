//! Per-modality redaction shape: the [`Redactable`] extension trait
//! plus per-modality strategy + replacement types.
//!
//! Adds redaction-side bindings (`Strategy` + `Replacement`) atop the
//! atomic [`Modality`] marker. Code that drives redaction (the engine
//! orchestrator, the policy evaluator) bounds on `M: Redactable`;
//! code that just walks modality-generic data structures bounds on
//! bare `Modality`.
//!
//! [`Modality`]: nvisy_core::modality::Modality

pub mod replacement;
pub mod strategy;

use std::fmt::Debug;

use nvisy_core::modality::{Audio, Image, Modality, Tabular, Text};

pub use self::replacement::{TabularReplacement, TextReplacement};
pub use self::strategy::{
    AudioMethodTag, AudioStrategy, ImageMethodTag, ImageStrategy, TabularStrategy, TextStrategy,
};

/// What a redacted output leaks about the original it replaced.
///
/// Variants are ordered from most-leaky to least-leaky, so
/// `Recoverable < Partial < Irrecoverable`. Used today for operator
/// understanding and policy authoring; future conflict-resolution
/// passes may consult the ordering when two methods compete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LeakProfile {
    /// The original value is recoverable from the output given the
    /// right metadata (encryption key, token vault, pseudonym map,
    /// or the candidate entity list against a hash).
    Recoverable,
    /// The original value is gone, but observable shape leaks:
    /// position, length, bounding box, cell coordinates, or a known
    /// silence on the timeline.
    Partial,
    /// No trace of the original value or its shape remains in the
    /// output.
    Irrecoverable,
}

/// Shape every per-modality redaction strategy implements.
pub trait RedactionStrategy {
    /// What the strategy's output leaks about the original.
    fn leak_profile(&self) -> LeakProfile;

    /// Whether the strategy is reversible — true iff the leak
    /// profile is [`LeakProfile::Recoverable`].
    fn is_reversible(&self) -> bool {
        self.leak_profile() == LeakProfile::Recoverable
    }
}

/// Redaction-side extension of [`Modality`].
///
/// Adds the strategy + replacement-record shape redaction passes
/// need. Each modality declares the strategies that make sense for
/// its data (text picks mask/replace/encrypt/etc., image picks
/// blur/block/pixelate, audio picks silence/remove, tabular picks
/// clear/drop-column) and the replacement record shape audited per
/// applied redaction.
///
/// [`Modality`]: nvisy_core::modality::Modality
pub trait Redactable: Modality {
    /// The modality's redaction strategy enum.
    type Strategy: RedactionStrategy + Clone + Debug + Default + PartialEq + Send + Sync + 'static;

    /// What an applied redaction wrote back at the entity's
    /// location. Text/Tabular carry the replacement string; Image
    /// /Audio carry the method tag only (the substitution is a
    /// binary transform whose parameters live on `Strategy`).
    type Replacement: Clone + Debug + PartialEq + Send + Sync + 'static;
}

impl Redactable for Text {
    type Replacement = TextReplacement;
    type Strategy = TextStrategy;
}

impl Redactable for Image {
    type Replacement = ImageMethodTag;
    type Strategy = ImageStrategy;
}

impl Redactable for Audio {
    type Replacement = AudioMethodTag;
    type Strategy = AudioStrategy;
}

impl Redactable for Tabular {
    type Replacement = TabularReplacement;
    type Strategy = TabularStrategy;
}
