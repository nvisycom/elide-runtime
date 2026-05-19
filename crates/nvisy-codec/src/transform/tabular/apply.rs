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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::TextOutput;

    fn redaction(start: usize, end: usize, replacement: &str) -> TabularRedaction {
        TabularRedaction::new(start, end, TextOutput::replace(replacement))
    }

    #[test]
    fn single_replacement() {
        let mut s = String::from("hello world");
        apply_tabular_redactions(&mut s, &[redaction(0, 5, "[X]")], "test").unwrap();
        assert_eq!(s, "[X] world");
    }

    #[test]
    fn right_to_left_application() {
        let mut s = String::from("aaa bbb ccc");
        let rs = vec![redaction(0, 3, "[A]"), redaction(8, 11, "[C]")];
        apply_tabular_redactions(&mut s, &rs, "test").unwrap();
        assert_eq!(s, "[A] bbb [C]");
    }

    #[test]
    fn remove_output() {
        let mut s = String::from("hello world");
        apply_tabular_redactions(
            &mut s,
            &[TabularRedaction::new(5, 11, TextOutput::Remove)],
            "test",
        )
        .unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn out_of_bounds_clipped() {
        let mut s = String::from("short");
        apply_tabular_redactions(&mut s, &[redaction(0, 999, "[X]")], "test").unwrap();
        assert_eq!(s, "[X]");
    }

    #[test]
    fn mid_character_rejected() {
        let mut s = String::from("héllo"); // 'é' is 2 bytes
        let err = apply_tabular_redactions(&mut s, &[redaction(0, 2, "[X]")], "test").unwrap_err();
        assert!(err.to_string().contains("mid-character"));
    }
}
