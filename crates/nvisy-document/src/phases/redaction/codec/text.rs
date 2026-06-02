//! `TextStrategy → TextRedaction` conversion.

use nvisy_codec::handler::TextRedaction;
use nvisy_core::Result;
use nvisy_core::entity::EntityKind;

use super::{TextOutputMethod, text_output_from};
use crate::policy::TextStrategy;

/// Convert a [`TextStrategy`] + the entity's original value into a
/// codec [`TextRedaction`].
pub(crate) fn to_text_redaction(
    strategy: &TextStrategy,
    original: &str,
    entity_kind: EntityKind,
) -> Result<TextRedaction> {
    let method = match strategy {
        TextStrategy::Replace { placeholder } => TextOutputMethod::Replace { placeholder },
        TextStrategy::Mask { mask_char } => TextOutputMethod::Mask {
            mask_char: *mask_char,
        },
        TextStrategy::Hash => TextOutputMethod::Hash,
        TextStrategy::Remove => TextOutputMethod::Remove,
        TextStrategy::Encrypt { key_id } => TextOutputMethod::Encrypt { key_id },
    };
    let output = text_output_from(method, original, entity_kind)?;
    Ok(TextRedaction::new(output))
}
