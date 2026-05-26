//! Shared byte-level string redaction helper used by every
//! text-family handler (TXT, JSON, HTML, the per-page text inside
//! the PDF rich handler) plus the tabular cell handlers (CSV, XLSX
//! cells are flat strings).
//!
//! Replaces `content[start..end]` with `value`. Clamps offsets to
//! `content.len()`; errors when either offset falls mid-character.

use nvisy_core::Error;

/// Replace `buf[start..end]` with `value` in place.
///
/// Returns an error if either offset falls mid-character. Offsets are
/// clamped to `buf.len()`; an empty replacement against an empty range
/// is a no-op.
pub(crate) fn replace_range(
    buf: &mut String,
    value: &str,
    start: usize,
    end: usize,
    target: &'static str,
) -> Result<(), Error> {
    let s = start.min(buf.len());
    let e = end.min(buf.len());
    if s >= e {
        return Ok(());
    }
    if !buf.is_char_boundary(s) || !buf.is_char_boundary(e) {
        return Err(Error::validation(
            format!(
                "redaction offset falls mid-character \
                 (start={start}, end={end}, len={})",
                buf.len()
            ),
            target,
        ));
    }
    buf.replace_range(s..e, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_replacement() {
        let mut s = String::from("hello world");
        replace_range(&mut s, "[X]", 0, 5, "test").unwrap();
        assert_eq!(s, "[X] world");
    }

    #[test]
    fn remove_empty_value() {
        let mut s = String::from("hello world");
        replace_range(&mut s, "", 5, 11, "test").unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn out_of_bounds_clipped() {
        let mut s = String::from("short");
        replace_range(&mut s, "[X]", 0, 999, "test").unwrap();
        assert_eq!(s, "[X]");
    }

    #[test]
    fn mid_character_rejected() {
        let mut s = String::from("héllo"); // 'é' is 2 bytes
        let err = replace_range(&mut s, "[X]", 0, 2, "test").unwrap_err();
        assert!(err.to_string().contains("mid-character"));
    }
}
