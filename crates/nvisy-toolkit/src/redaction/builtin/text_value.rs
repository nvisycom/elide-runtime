//! Shared helper for text operators: read the original substring
//! at an entity's location, returning `""` for out-of-bounds or
//! mid-char offsets.

use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};

/// Borrow the substring at `entity.location` from `source`. Returns
/// `""` when the range is empty, out of bounds, or splits a UTF-8
/// character boundary.
pub(super) fn read_value<'a>(entity: &Entity<Text>, source: &'a TextData) -> &'a str {
    let text = source.text.as_str();
    let start = entity.location.start.min(text.len());
    let end = entity.location.end.min(text.len());
    if start >= end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return "";
    }
    &text[start..end]
}
