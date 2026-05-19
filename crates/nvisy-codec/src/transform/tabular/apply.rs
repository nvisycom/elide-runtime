//! Byte-level helper for applying a batch of [`TabularRedaction`]s to a
//! single cell's value in place.

use std::cmp::Reverse;

use nvisy_core::Error;

use super::instruction::TabularRedaction;

/// Apply a slice of cell-scoped redactions to `cell` in place.
///
/// Redactions are sorted right-to-left so earlier byte offsets stay
/// valid as later ones are replaced. Returns an error if any offset
/// falls mid-character.
///
/// The slice must not contain overlapping ranges — that invariant is
/// owned by [`Redactions`] on insert.
///
/// [`Redactions`]: crate::transform::Redactions
pub(crate) fn apply_tabular_redactions(
    cell: &mut String,
    redactions: &[TabularRedaction],
    target: &'static str,
) -> Result<(), Error> {
    let mut items: Vec<&TabularRedaction> = redactions.iter().collect();
    items.sort_by_key(|r| Reverse(r.start));

    for r in items {
        let value = r.output.replacement_value().unwrap_or_default();
        let s = r.start.min(cell.len());
        let e = r.end.min(cell.len());
        if s >= e {
            continue;
        }
        if !cell.is_char_boundary(s) || !cell.is_char_boundary(e) {
            return Err(Error::validation(
                format!(
                    "redaction offset falls mid-character \
                     (start={}, end={}, len={})",
                    r.start,
                    r.end,
                    cell.len()
                ),
                target,
            ));
        }
        cell.replace_range(s..e, value);
    }

    Ok(())
}
