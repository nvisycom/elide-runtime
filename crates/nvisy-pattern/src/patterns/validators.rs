//! Checksum and format validators for detected entity values.
//!
//! These functions are referenced by pattern definitions in `patterns.json`
//! and are also used directly by the checksum detection action.

/// Validate a US Social Security Number.
pub fn validate_ssn(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let area: u32 = match parts[0].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let group: u32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let serial: u32 = match parts[2].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    area > 0 && area < 900 && area != 666 && group > 0 && serial > 0
}

/// Luhn check algorithm for credit card validation.
pub fn luhn_check(num: &str) -> bool {
    let digits: String = num.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let mut sum = 0u32;
    let mut alternate = false;
    for ch in digits.chars().rev() {
        let mut n = ch.to_digit(10).unwrap_or(0);
        if alternate {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        alternate = !alternate;
    }
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_ssn ---

    #[test]
    fn ssn_valid() {
        assert!(validate_ssn("123-45-6789"));
        assert!(validate_ssn("001-01-0001"));
        assert!(validate_ssn("899-99-9999"));
    }

    #[test]
    fn ssn_invalid_area_zero() {
        assert!(!validate_ssn("000-45-6789"));
    }

    #[test]
    fn ssn_invalid_area_666() {
        assert!(!validate_ssn("666-45-6789"));
    }

    #[test]
    fn ssn_invalid_area_900_plus() {
        assert!(!validate_ssn("900-45-6789"));
        assert!(!validate_ssn("999-45-6789"));
    }

    #[test]
    fn ssn_invalid_group_zero() {
        assert!(!validate_ssn("123-00-6789"));
    }

    #[test]
    fn ssn_invalid_serial_zero() {
        assert!(!validate_ssn("123-45-0000"));
    }

    #[test]
    fn ssn_wrong_format() {
        assert!(!validate_ssn("12345-6789"));
        assert!(!validate_ssn("123456789"));
        assert!(!validate_ssn("abc-de-fghi"));
        assert!(!validate_ssn(""));
    }

    // --- luhn_check ---

    #[test]
    fn luhn_valid_card_numbers() {
        // Standard test numbers
        assert!(luhn_check("4539 1488 0343 6467"));
        assert!(luhn_check("4539148803436467"));
        assert!(luhn_check("4539-1488-0343-6467"));
    }

    #[test]
    fn luhn_invalid_card_numbers() {
        assert!(!luhn_check("4539 1488 0343 6466"));
        assert!(!luhn_check("1234567890123456"));
    }

    #[test]
    fn luhn_empty_input() {
        assert!(!luhn_check(""));
    }

    #[test]
    fn luhn_non_digit_input() {
        assert!(!luhn_check("abcdef"));
    }

    #[test]
    fn luhn_single_zero() {
        assert!(luhn_check("0"));
    }
}
