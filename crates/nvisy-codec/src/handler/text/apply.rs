//! Byte-level helper for applying a single [`TextRedaction`] to a
//! string in place. Shared across the text-family handlers (TXT, JSON,
//! HTML, and the per-page text in [`RichTextHandler`]).
//!
//! [`RichTextHandler`]: crate::handler::rich::RichTextHandler

use nvisy_core::Error;

use crate::handler::TextRedaction;

/// Apply a single redaction to `content` in place.
///
/// Returns an error if the redaction's byte offsets fall mid-character.
/// Out-of-range offsets are silently clipped to `content.len()`.
pub(crate) fn apply_text_redaction(
    content: &mut String,
    redaction: &TextRedaction,
    target: &'static str,
) -> Result<(), Error> {
    let value = redaction.output.replacement_value().unwrap_or_default();
    let s = redaction.start.min(content.len());
    let e = redaction.end.min(content.len());
    if s >= e {
        return Ok(());
    }
    if !content.is_char_boundary(s) || !content.is_char_boundary(e) {
        return Err(Error::validation(
            format!(
                "redaction offset falls mid-character \
                 (start={}, end={}, len={})",
                redaction.start,
                redaction.end,
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

    fn redaction(start: usize, end: usize, replacement: &str) -> TextRedaction {
        TextRedaction::new(start, end, TextOutput::replace(replacement))
    }

    #[test]
    fn single_replacement() {
        let mut s = String::from("hello world");
        apply_text_redaction(&mut s, &redaction(0, 5, "[X]"), "test").unwrap();
        assert_eq!(s, "[X] world");
    }

    #[test]
    fn remove_output() {
        let mut s = String::from("hello world");
        apply_text_redaction(
            &mut s,
            &TextRedaction::new(5, 11, TextOutput::Remove),
            "test",
        )
        .unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn out_of_bounds_clipped() {
        let mut s = String::from("short");
        apply_text_redaction(&mut s, &redaction(0, 999, "[X]"), "test").unwrap();
        assert_eq!(s, "[X]");
    }

    #[test]
    fn mid_character_rejected() {
        let mut s = String::from("héllo"); // 'é' is 2 bytes
        let err = apply_text_redaction(&mut s, &redaction(0, 2, "[X]"), "test").unwrap_err();
        assert!(err.to_string().contains("mid-character"));
    }
}
