//! Luhn checksum validator.
//!
//! Implements the [Luhn algorithm](https://en.wikipedia.org/wiki/Luhn_algorithm)
//! used to validate credit/debit card numbers and other identification
//! numbers.  Non-digit characters (spaces, dashes) are stripped before
//! the check.

/// Return `true` if `num` passes the Luhn checksum.
///
/// All non-digit characters are ignored, so `"4539 1488 0343 6467"`,
/// `"4539-1488-0343-6467"`, and `"4539148803436467"` are equivalent.
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

    #[test]
    fn valid_card_numbers() {
        assert!(luhn_check("4539 1488 0343 6467"));
        assert!(luhn_check("4539148803436467"));
        assert!(luhn_check("4539-1488-0343-6467"));
    }

    #[test]
    fn invalid_card_numbers() {
        assert!(!luhn_check("4539 1488 0343 6466"));
        assert!(!luhn_check("1234567890123456"));
    }

    #[test]
    fn empty_input() {
        assert!(!luhn_check(""));
    }

    #[test]
    fn non_digit_input() {
        assert!(!luhn_check("abcdef"));
    }

    #[test]
    fn single_zero() {
        assert!(luhn_check("0"));
    }
}
