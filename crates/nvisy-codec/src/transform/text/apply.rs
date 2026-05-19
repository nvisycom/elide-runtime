//! Byte-level helper for applying a batch of [`TextRedaction`]s
//! to a string in place.

use std::cmp::Reverse;

use nvisy_core::Error;

use super::instruction::TextRedaction;

/// Apply a slice of redactions to `content` in place.
///
/// Redactions are sorted right-to-left so that earlier byte offsets
/// remain valid as later ones are replaced. Returns an error if any
/// offset falls mid-character.
///
/// The slice must not contain overlapping ranges — that invariant is
/// owned by [`Redactions`] on insert.
///
/// [`Redactions`]: crate::transform::Redactions
pub(crate) fn apply_text_redactions(
    content: &mut String,
    redactions: &[TextRedaction],
    target: &'static str,
) -> Result<(), Error> {
    let mut items: Vec<&TextRedaction> = redactions.iter().collect();
    items.sort_by_key(|r| Reverse(r.start));

    for r in items {
        let value = r.output.replacement_value().unwrap_or_default();
        let s = r.start.min(content.len());
        let e = r.end.min(content.len());
        if s >= e {
            continue;
        }
        if !content.is_char_boundary(s) || !content.is_char_boundary(e) {
            return Err(Error::validation(
                format!(
                    "redaction offset falls mid-character \
                     (start={}, end={}, len={})",
                    r.start,
                    r.end,
                    content.len()
                ),
                target,
            ));
        }
        content.replace_range(s..e, value);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::TextOutput;

    fn redaction(start: usize, end: usize, replacement: &str) -> TextRedaction {
        TextRedaction::new(start, end, TextOutput::replace(replacement))
    }

    #[test]
    fn single_replacement() {
        let mut s = String::from("hello world");
        apply_text_redactions(&mut s, &[redaction(0, 5, "[X]")], "test").unwrap();
        assert_eq!(s, "[X] world");
    }

    #[test]
    fn right_to_left_application() {
        let mut s = String::from("aaa bbb ccc");
        let rs = vec![redaction(0, 3, "[A]"), redaction(8, 11, "[C]")];
        apply_text_redactions(&mut s, &rs, "test").unwrap();
        assert_eq!(s, "[A] bbb [C]");
    }

    #[test]
    fn remove_output() {
        let mut s = String::from("hello world");
        apply_text_redactions(
            &mut s,
            &[TextRedaction::new(5, 11, TextOutput::Remove)],
            "test",
        )
        .unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn out_of_bounds_clipped() {
        let mut s = String::from("short");
        apply_text_redactions(&mut s, &[redaction(0, 999, "[X]")], "test").unwrap();
        assert_eq!(s, "[X]");
    }

    #[test]
    fn mid_character_rejected() {
        let mut s = String::from("héllo"); // 'é' is 2 bytes
        let err = apply_text_redactions(&mut s, &[redaction(0, 2, "[X]")], "test").unwrap_err();
        assert!(err.to_string().contains("mid-character"));
    }
}
