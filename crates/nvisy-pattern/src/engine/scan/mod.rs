//! Internal scan machinery: per-pattern compiled entries, the per-match
//! intermediate type, and the per-phase scan + dedup logic.
//!
//! None of these types are part of the public API. The orchestration
//! lives in [`PatternEngine::scan_entities`].
//!
//! [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities

pub(super) mod dedup;
pub(super) mod entries;
pub(super) mod pattern_match;
pub(super) mod phases;
