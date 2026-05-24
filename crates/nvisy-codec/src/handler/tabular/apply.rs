//! Helper for applying a single [`TabularRedaction`] to one cell's
//! value in place.

use nvisy_core::Error;

use crate::handler::TabularRedaction;

/// Apply a single redaction to `cell` in place, restricted to byte
/// range `start..end` (clamped to the cell's length). Returns an
/// error if either offset falls mid-character.
///
/// The intra-cell byte range comes from the redaction's containing
/// [`TabularLocation`] under the `(location, redaction)` shape — not
/// from the redaction itself.
///
/// [`TabularLocation`]: nvisy_ontology::entity::TabularLocation
pub fn apply_tabular_redaction(
    cell: &mut String,
    redaction: &TabularRedaction,
    start: usize,
    end: usize,
    target: &'static str,
) -> Result<(), Error> {
    let value = redaction.output.replacement_value().unwrap_or_default();
    let s = start.min(cell.len());
    let e = end.min(cell.len());
    if s >= e {
        return Ok(());
    }
    if !cell.is_char_boundary(s) || !cell.is_char_boundary(e) {
        return Err(Error::validation(
            format!(
                "redaction offset falls mid-character \
                 (start={start}, end={end}, len={})",
                cell.len()
            ),
            target,
        ));
    }
    cell.replace_range(s..e, value);
    Ok(())
}
