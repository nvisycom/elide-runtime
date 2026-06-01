//! `TabularStrategy → TabularRedaction` conversion.

use nvisy_codec::handler::{TabularRedaction, TextOutput};
use nvisy_core::Result;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::policy::TabularStrategy;

use super::{TextOutputMethod, text_output_from, unsupported};

/// Convert a [`TabularStrategy`] + the entity's original cell value
/// into a codec [`TabularRedaction`].
///
/// Tabular cells share the `TextOutput` shape with `Text`; the
/// extra `Clear` variant maps to an empty-string `Replace`.
pub(crate) fn to_tabular_redaction(
    strategy: &TabularStrategy,
    original: &str,
    entity_kind: EntityKind,
) -> Result<TabularRedaction> {
    let output = match strategy {
        TabularStrategy::Replace { placeholder } => text_output_from(
            TextOutputMethod::Replace { placeholder },
            original,
            entity_kind,
        )?,
        TabularStrategy::Mask { mask_char } => text_output_from(
            TextOutputMethod::Mask {
                mask_char: *mask_char,
            },
            original,
            entity_kind,
        )?,
        TabularStrategy::Hash => text_output_from(TextOutputMethod::Hash, original, entity_kind)?,
        TabularStrategy::Clear => TextOutput::Replace {
            replacement: String::new(),
        },
        TabularStrategy::Encrypt { key_id } => {
            text_output_from(TextOutputMethod::Encrypt { key_id }, original, entity_kind)?
        }
        TabularStrategy::DropColumn => return Err(unsupported("drop_column")),
        TabularStrategy::DropRow => return Err(unsupported("drop_row")),
    };
    Ok(TabularRedaction::new(output))
}
