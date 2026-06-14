//! IBAN checksum validator (ISO 13616).
//!
//! Rearranges the IBAN so the country code and check digits move to the
//! end, converts letters to numbers (A=10 … Z=35), and verifies that
//! the resulting number mod 97 equals 1.

/// Return `true` if `value` passes the ISO 13616 mod-97 IBAN check.
///
/// Whitespace and dashes are stripped before validation.
pub fn iban(value: &str) -> bool {
    let cleaned: String = value
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '-')
        .collect();

    if cleaned.len() < 5 {
        return false;
    }

    // All characters must be alphanumeric ASCII.
    if !cleaned.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }

    // Move first 4 characters (country code + check digits) to the end.
    let rearranged = format!("{}{}", &cleaned[4..], &cleaned[..4]);

    // Convert letters to two-digit numbers (A=10 … Z=35) and compute mod 97.
    let mut remainder: u32 = 0;
    for ch in rearranged.chars() {
        let digit_val = if ch.is_ascii_digit() {
            ch.to_digit(10).unwrap()
        } else {
            // A=10, B=11, … Z=35
            (ch.to_ascii_uppercase() as u32) - ('A' as u32) + 10
        };

        // For two-digit values (>=10) we need to shift by two decimal places.
        if digit_val >= 10 {
            remainder = (remainder * 100 + digit_val) % 97;
        } else {
            remainder = (remainder * 10 + digit_val) % 97;
        }
    }

    remainder == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ibans() {
        // GB, DE, FR examples from Wikipedia.
        assert!(iban("GB29 NWBK 6016 1331 9268 19"));
        assert!(iban("DE89370400440532013000"));
        assert!(iban("FR76 3000 6000 0112 3456 7890 189"));
    }

    #[test]
    fn invalid_check_digits() {
        assert!(!iban("GB29 NWBK 6016 1331 9268 18"));
        assert!(!iban("DE00370400440532013000"));
    }

    #[test]
    fn too_short() {
        assert!(!iban("GB29"));
        assert!(!iban(""));
    }

    #[test]
    fn non_alphanumeric() {
        assert!(!iban("GB29!NWBK60161331926819"));
    }

    #[test]
    fn strips_whitespace_and_dashes() {
        assert!(iban("GB29-NWBK-6016-1331-9268-19"));
        assert!(iban("  GB29 NWBK 6016 1331 9268 19  "));
    }
}
