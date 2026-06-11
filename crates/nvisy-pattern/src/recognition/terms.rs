//! [`Terms`]: a literal-string list, the term source for
//! [`Dictionary`].
//!
//! [`Dictionary`]: crate::Dictionary
//!
//! A `Terms` value is the bag of literals the recognizer's
//! Aho-Corasick automaton scans for. Construct it from any common
//! shape:
//!
//! - [`Terms::from`] — `Vec<String>`, `&[&str]`, or `[&str; N]`
//! - [`Terms::from_text`] — one term per line, trimmed, with
//!   `#`-prefixed comments and blank lines skipped
//! - [`Terms::from_csv`] — every non-empty cell across every row
//!   becomes a term; each term remembers its source column index
//!   so dictionaries can apply per-column confidence overrides

use std::io::Cursor;

use nvisy_core::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Literal term list. Each term carries the **column index** it
/// came from (CSV column number, 0-based; non-CSV sources always
/// use column `0`). The column index is the join key for
/// [`Dictionary::column_scores`] per-column overrides.
///
/// JSON-transparent: serialises to / deserialises from a JSON array
/// of `[term, column]` pairs.
///
/// [`Dictionary::column_scores`]: crate::Dictionary::column_scores
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Terms(Vec<TermEntry>);

/// One entry in a [`Terms`] list: the literal plus the column it
/// was loaded from. Serde-renamed so the wire shape is the compact
/// tuple `[term, column]` rather than a verbose object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TermEntry {
    /// The literal scanned for.
    pub term: String,
    /// CSV column the term came from (0-based). `0` for any
    /// non-CSV source.
    #[serde(default)]
    pub column: u16,
}

impl Terms {
    /// Construct an empty term list.
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Borrow the inner entries.
    #[must_use]
    pub fn entries(&self) -> &[TermEntry] {
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

    /// Consume into the inner entries.
    #[must_use]
    pub fn into_inner(self) -> Vec<TermEntry> {
        self.0
    }

    /// Parse terms from plain-text bytes — one term per line.
    /// Each line is trimmed; empty lines and lines starting with `#`
    /// are skipped. Every term gets column `0`.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the input is not valid UTF-8.
    pub fn from_text(bytes: &[u8]) -> Result<Self, Error> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| Error::validation(format!("terms text: {e}"), "nvisy-pattern"))?;
        let entries: Vec<TermEntry> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| TermEntry {
                term: line.to_owned(),
                column: 0,
            })
            .collect();
        Ok(Self(entries))
    }

    /// Parse terms from CSV bytes. Every non-empty cell across every
    /// row becomes a term, and each term remembers the (0-based)
    /// column index it came from so a [`Dictionary`] can apply
    /// per-column confidence overrides via
    /// [`Dictionary::column_scores`].
    ///
    /// # Errors
    ///
    /// Returns a validation error when the CSV is malformed.
    ///
    /// [`Dictionary`]: crate::Dictionary
    /// [`Dictionary::column_scores`]: crate::Dictionary::column_scores
    pub fn from_csv(bytes: &[u8]) -> Result<Self, Error> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(bytes));
        let mut entries = Vec::new();
        for row in reader.records() {
            let row =
                row.map_err(|e| Error::validation(format!("terms CSV: {e}"), "nvisy-pattern"))?;
            for (col_idx, cell) in row.iter().enumerate() {
                let trimmed = cell.trim();
                if !trimmed.is_empty() {
                    entries.push(TermEntry {
                        term: trimmed.to_owned(),
                        column: u16::try_from(col_idx).unwrap_or(u16::MAX),
                    });
                }
            }
        }
        Ok(Self(entries))
    }
}

impl From<Vec<String>> for Terms {
    fn from(terms: Vec<String>) -> Self {
        Self(
            terms
                .into_iter()
                .map(|term| TermEntry { term, column: 0 })
                .collect(),
        )
    }
}

impl From<&[&str]> for Terms {
    fn from(terms: &[&str]) -> Self {
        Self(
            terms
                .iter()
                .map(|s| TermEntry {
                    term: (*s).to_owned(),
                    column: 0,
                })
                .collect(),
        )
    }
}

impl<const N: usize> From<[&str; N]> for Terms {
    fn from(terms: [&str; N]) -> Self {
        Self(
            terms
                .iter()
                .map(|s| TermEntry {
                    term: (*s).to_owned(),
                    column: 0,
                })
                .collect(),
        )
    }
}

impl<const N: usize> From<[String; N]> for Terms {
    fn from(terms: [String; N]) -> Self {
        Self(
            terms
                .into_iter()
                .map(|term| TermEntry { term, column: 0 })
                .collect(),
        )
    }
}
