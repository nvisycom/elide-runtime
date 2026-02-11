//! Built-in dictionary data for name and term matching.
//!
//! Dictionaries are embedded at compile time via `include_str!()` and
//! loaded lazily on first access.

use std::sync::LazyLock;

static FIRST_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| {
    parse_dictionary(include_str!("../../assets/dictionaries/first_names.txt"))
});

static LAST_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| {
    parse_dictionary(include_str!("../../assets/dictionaries/last_names.txt"))
});

static MEDICAL_TERMS: LazyLock<Vec<String>> = LazyLock::new(|| {
    parse_dictionary(include_str!("../../assets/dictionaries/medical_terms.txt"))
});

/// Load a built-in dictionary by name.
///
/// Names are prefixed with `"builtin:"` — e.g. `"builtin:first_names"`,
/// `"builtin:last_names"`, `"builtin:medical_terms"`.
///
/// Returns `None` if the name is not recognized.
pub fn get_builtin(name: &str) -> Option<&'static [String]> {
    match name {
        "builtin:first_names" => Some(&FIRST_NAMES),
        "builtin:last_names" => Some(&LAST_NAMES),
        "builtin:medical_terms" => Some(&MEDICAL_TERMS),
        _ => None,
    }
}

fn parse_dictionary(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}
