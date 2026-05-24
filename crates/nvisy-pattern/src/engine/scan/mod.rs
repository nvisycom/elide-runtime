//! Internal scan machinery: per-pattern compiled entries, the per-match
//! intermediate type, and the per-phase scan logic.
//!
//! None of these types are part of the public API. The orchestration
//! lives in [`PatternEngine::scan_entities`]; cross-recognizer
//! deduplication is the engine layer's responsibility, not this
//! crate's.
//!
//! [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities

pub(super) mod enhancer;
pub(crate) mod entries;
pub(super) mod pattern_match;
pub(super) mod phases;
