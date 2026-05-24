//! Byte-level helper for applying a single [`TextRedaction`] to a
//! string in place. Shared across the text-family handlers (TXT, JSON,
//! HTML, and the per-page text in [`RichTextHandler`]).
//!
//! The byte range `start..end` comes from the redaction's containing
//! [`TextLocation`] under the `(location, redaction)` shape — not from
//! the redaction itself. Callers translate the location's
//! document-absolute offsets into span-relative offsets before
//! calling.
//!
//! [`RichTextHandler`]: crate::handler::rich::RichTextHandler
//! [`TextLocation`]: nvisy_ontology::entity::TextLocation

use nvisy_core::Error;

use crate::handler::TextRedaction;

/// Apply a single redaction to `content` in place, restricted to byte
/// range `start..end` (clamped to `content.len()`). Returns an error
/// if either offset falls mid-character.
pub fn apply_text_redaction(
    content: &mut String,
    redaction: &TextRedaction,
    start: usize,
    end: usize,
    target: &'static str,
) -> Result<(), Error> {
    let value = redaction.output.replacement_value().unwrap_or_default();
    let s = start.min(content.len());
    let e = end.min(content.len());
    if s >= e {
        return Ok(());
    }
    if !content.is_char_boundary(s) || !content.is_char_boundary(e) {
        return Err(Error::validation(
            format!(
                "redaction offset falls mid-character \
                 (start={start}, end={end}, len={})",
                content.len()
            ),
            target,
        ));
    }
    content.replace_range(s..e, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::TextOutput;

    fn redaction(replacement: &str) -> TextRedaction {
        TextRedaction::new(TextOutput::replace(replacement))
    }

    #[test]
    fn single_replacement() {
        let mut s = String::from("hello world");
        apply_text_redaction(&mut s, &redaction("[X]"), 0, 5, "test").unwrap();
        assert_eq!(s, "[X] world");
    }

    #[test]
    fn remove_output() {
        let mut s = String::from("hello world");
        apply_text_redaction(
            &mut s,
            &TextRedaction::new(TextOutput::Remove),
            5,
            11,
            "test",
        )
        .unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn out_of_bounds_clipped() {
        let mut s = String::from("short");
        apply_text_redaction(&mut s, &redaction("[X]"), 0, 999, "test").unwrap();
        assert_eq!(s, "[X]");
    }

    #[test]
    fn mid_character_rejected() {
        let mut s = String::from("héllo"); // 'é' is 2 bytes
        let err = apply_text_redaction(&mut s, &redaction("[X]"), 0, 2, "test").unwrap_err();
        assert!(err.to_string().contains("mid-character"));
    }
}
