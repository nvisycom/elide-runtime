//! [`AudioRedaction`]: the operator spec an audio-modality policy
//! rule carries.
//!
//! Each variant mirrors an elide built-in operator the engine
//! constructs at apply time:
//!
//! - [`AudioRedaction::Erase`] → `elide::redaction::operators::Erase`
//!   cuts the interval out, shortening the clip.
//! - [`AudioRedaction::Keep`] → `elide::redaction::operators::Keep`
//!   passes the interval through unchanged.
//! - [`AudioRedaction::Silence`] →
//!   `elide::redaction::operators::Silence` zeroes the interval in
//!   place, preserving the clip's length and downstream timing.
//! - [`AudioRedaction::Beep`] → `elide::redaction::operators::Beep`
//!   overlays a tone over the interval (the broadcast-bleep
//!   treatment).

use elide_core::modality::audio::Waveform;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operator spec a `redact` audio rule carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioRedaction {
    /// Cut the matched interval out, shortening the clip.
    Erase,
    /// Pass the interval through unchanged.
    Keep,
    /// Zero the matched interval in place, preserving the clip
    /// length and the timing of everything after it.
    Silence,
    /// Overlay a tone over the matched interval (the broadcast
    /// "bleep"); preserves duration.
    Beep {
        /// Tone frequency in Hertz. Default 1 kHz, the broadcast
        /// convention.
        #[serde(default = "default_beep_hz")]
        hz: f32,
        /// Peak amplitude in `0.0..=1.0` of full scale. Default 0.5:
        /// audible but not painful, and never clips.
        #[serde(default = "default_beep_amplitude")]
        amplitude: f32,
        /// Tone shape. Default sine.
        #[serde(default = "default_beep_waveform")]
        waveform: Waveform,
    },
}

fn default_beep_hz() -> f32 {
    1000.0
}

fn default_beep_amplitude() -> f32 {
    0.5
}

fn default_beep_waveform() -> Waveform {
    Waveform::Sine
}
