//! Post-generation reshaping: format-preserving + length-preserving
//! transforms applied to fake values before they reach the caller.

/// Walk `original` left-to-right, copy non-digit / non-letter
/// characters straight into the output, and pull the next
/// "interesting" character from `value` to fill each digit / letter
/// slot. Once `value` is exhausted, remaining slots are dropped.
///
/// Used by format-preserving mode for digit-shaped kinds
/// (phone, postal code). Letters are preserved in case
/// the original used alphanumeric structure (UK postcodes:
/// `SW1A 1AA`).
pub(crate) fn reshape_to_original(value: &str, original: &str) -> String {
    let mut out = String::with_capacity(original.len());
    let mut fill = value.chars().filter(|c| c.is_alphanumeric());
    for ch in original.chars() {
        if ch.is_alphanumeric() {
            if let Some(next) = fill.next() {
                out.push(next);
            } else {
                break;
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Trim `value` to `target` characters, padding with `0` if shorter.
/// Acts on Unicode scalars, not bytes — safe for multibyte input
/// such as a Japanese kanji-formatted DOB.
pub(crate) fn clip_or_pad(value: &str, target: usize) -> String {
    let mut chars: Vec<char> = value.chars().collect();
    if chars.len() == target {
        return value.to_owned();
    }
    if chars.len() > target {
        chars.truncate(target);
    } else {
        chars.resize(target, '0');
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reshape_keeps_original_separators() {
        assert_eq!(
            reshape_to_original("5551234567", "555-123-4567"),
            "555-123-4567"
        );
        assert_eq!(reshape_to_original("12345", "12 345"), "12 345");
    }

    #[test]
    fn reshape_drops_extra_digits_when_value_too_long() {
        assert_eq!(reshape_to_original("999999999", "12-34"), "99-99");
    }

    #[test]
    fn reshape_stops_when_value_runs_out() {
        assert_eq!(reshape_to_original("99", "555-12"), "99");
    }

    #[test]
    fn clip_truncates_when_too_long() {
        assert_eq!(clip_or_pad("123456", 4), "1234");
    }

    #[test]
    fn pad_zeros_when_too_short() {
        assert_eq!(clip_or_pad("12", 4), "1200");
    }
}
