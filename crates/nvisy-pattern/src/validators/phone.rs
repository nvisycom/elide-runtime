//! Phone number structural validation.
//!
//! Validates that a regex-matched phone number has a plausible structure:
//! correct digit count and no obviously invalid prefixes.

/// Validate a phone number matched by regex.
///
/// Strips all non-digit characters, then checks:
/// - 7 to 15 digits (ITU-T E.164 range)
/// - Does not start with 0 when a country code is present (10+ digits)
pub fn validate_phone(value: &str) -> bool {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    let len = digits.len();

    // ITU-T E.164: minimum 7 digits (local), maximum 15 (international).
    if !(7..=15).contains(&len) {
        return false;
    }

    // International numbers (10+ digits) should not start with 0.
    if len >= 10 && digits.starts_with('0') {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_us_numbers() {
        assert!(validate_phone("+1-555-123-4567"));
        assert!(validate_phone("(555) 123-4567"));
        assert!(validate_phone("555.123.4567"));
        assert!(validate_phone("5551234567"));
    }

    #[test]
    fn valid_international() {
        assert!(validate_phone("+44 20 7946 0958"));
        assert!(validate_phone("+49 30 12345678"));
        assert!(validate_phone("+81 3 1234 5678"));
    }

    #[test]
    fn too_few_digits() {
        assert!(!validate_phone("12345"));
        assert!(!validate_phone("123-45"));
    }

    #[test]
    fn too_many_digits() {
        assert!(!validate_phone("1234567890123456"));
    }

    #[test]
    fn international_starting_with_zero() {
        assert!(!validate_phone("0123456789012"));
    }

    #[test]
    fn local_number_with_seven_digits() {
        assert!(validate_phone("123-4567"));
    }
}
