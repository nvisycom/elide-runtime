//! [`Terms`]: a literal-string list, the term source for
//! [`Dictionary`](crate::Dictionary).
//!
//! A `Terms` value is the bag of literals the recognizer's
//! Aho-Corasick automaton scans for. Construct it from any common
//! shape:
//!
//! - [`Terms::from`] — `Vec<String>`, `&[&str]`, or `[&str; N]`
//! - [`Terms::from_text`] — one term per line, trimmed, with
//!   `#`-prefixed comments and blank lines skipped
//! - [`Terms::from_csv`] — every non-empty cell across every row
//!   becomes a term

use std::io::Cursor;

use nvisy_core::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Literal term list. JSON-transparent: serialises to / deserialises
/// from a JSON array of strings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Terms(pub Vec<String>);

impl Terms {
    /// Construct an empty term list.
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Borrow the inner slice.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Number of terms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether this list contains no terms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Consume into the inner `Vec<String>`.
    #[must_use]
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }

    /// Parse terms from plain-text bytes — one term per line.
    /// Each line is trimmed; empty lines and lines starting with `#`
    /// are skipped.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the input is not valid UTF-8.
    pub fn from_text(bytes: &[u8]) -> Result<Self, Error> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| Error::validation(format!("terms text: {e}"), "nvisy-pattern"))?;
        let terms: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_owned)
            .collect();
        Ok(Self(terms))
    }

    /// Parse terms from CSV bytes. Every non-empty cell across every
    /// row becomes a term, useful when each row pairs a canonical
    /// name with one or more aliases.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the CSV is malformed.
    pub fn from_csv(bytes: &[u8]) -> Result<Self, Error> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(bytes));
        let mut terms = Vec::new();
        for row in reader.records() {
            let row =
                row.map_err(|e| Error::validation(format!("terms CSV: {e}"), "nvisy-pattern"))?;
            for cell in row.iter() {
                let trimmed = cell.trim();
                if !trimmed.is_empty() {
                    terms.push(trimmed.to_owned());
                }
            }
        }
        Ok(Self(terms))
    }
}

impl From<Vec<String>> for Terms {
    fn from(terms: Vec<String>) -> Self {
        Self(terms)
    }
}

impl From<&[&str]> for Terms {
    fn from(terms: &[&str]) -> Self {
        Self(terms.iter().map(|s| (*s).to_owned()).collect())
    }
}

impl<const N: usize> From<[&str; N]> for Terms {
    fn from(terms: [&str; N]) -> Self {
        Self(terms.iter().map(|s| (*s).to_owned()).collect())
    }
}

impl<const N: usize> From<[String; N]> for Terms {
    fn from(terms: [String; N]) -> Self {
        Self(terms.into_iter().collect())
    }
}
