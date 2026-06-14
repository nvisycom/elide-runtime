//! Phone number structural validation.
//!
//! Validates that a regex-matched phone number has a plausible structure:
//! correct digit count and no obviously invalid prefixes.

/// Return `true` if `value` has a plausible phone-number structure.
///
/// Strips all non-digit characters, then checks:
///
/// - 7 to 15 digits (ITU-T E.164 range)
/// - When the original begins with `+` (explicit E.164), the digits
///   must not start with 0 (no country code is `0…`). National formats
///   such as UK `020 7946 0958` keep their trunk-prefix zero and remain
///   valid.
pub fn phone(value: &str) -> bool {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    let len = digits.len();

    if !(7..=15).contains(&len) {
        return false;
    }

    if value.trim_start().starts_with('+') && digits.starts_with('0') {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_us_numbers() {
        assert!(phone("+1-555-123-4567"));
        assert!(phone("(555) 123-4567"));
        assert!(phone("555.123.4567"));
        assert!(phone("5551234567"));
    }

    #[test]
    fn valid_international() {
        assert!(phone("+44 20 7946 0958"));
        assert!(phone("+49 30 12345678"));
        assert!(phone("+81 3 1234 5678"));
    }

    #[test]
    fn too_few_digits() {
        assert!(!phone("12345"));
        assert!(!phone("123-45"));
    }

    #[test]
    fn too_many_digits() {
        assert!(!phone("1234567890123456"));
    }

    #[test]
    fn e164_starting_with_zero_rejected() {
        assert!(!phone("+0123456789012"));
    }

    #[test]
    fn national_format_with_trunk_zero_accepted() {
        // UK national format keeps the leading 0 trunk prefix.
        assert!(phone("020 7946 0958"));
        assert!(phone("0207946 0958"));
    }

    #[test]
    fn local_number_with_seven_digits() {
        assert!(phone("123-4567"));
    }
}
