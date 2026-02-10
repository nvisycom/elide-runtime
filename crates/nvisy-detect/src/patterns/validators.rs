//! Checksum and format validators for detected entity values.
//!
//! These functions are referenced by pattern definitions in `patterns.json`
//! and are also used directly by [`DetectChecksumAction`](crate::actions::detect_checksum::DetectChecksumAction).

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
