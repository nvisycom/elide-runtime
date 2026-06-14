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
use nvisy_core::primitive::Confidence;
use serde::Deserialize;

/// Literal term list. Each [`Term`] carries an optional source
/// column (set by [`Terms::from_csv`]) plus an optional per-term
/// score override. The column index is the join key for
/// [`Dictionary::scoring`] when it's [`Scoring::PerColumn`].
///
/// [`Dictionary::scoring`]: crate::Dictionary::scoring
/// [`Scoring::PerColumn`]: crate::Scoring::PerColumn
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(transparent)]
pub struct Terms(Vec<Term>);

/// One entry in a [`Terms`] list: the literal, the column it was
/// loaded from (when applicable), and an optional explicit score
/// that overrides the dictionary's [`Scoring`] policy for this
/// term.
///
/// Per-term score is `None` for the common path — the dictionary's
/// [`Scoring`] resolves the per-match score from the column.
/// Set `score` only for one-off exceptions (e.g. a term known to
/// be high-confidence even though its column is generally noisy).
///
/// Per-term column is `None` for non-CSV sources (plain text
/// lists, the `From<Vec<String>>` / array impls). `Some(i)` flags
/// a CSV cell from column `i`; the dictionary's
/// [`Scoring::PerColumn`] uses it to pick the per-column score.
///
/// [`Scoring`]: crate::Scoring
/// [`Scoring::PerColumn`]: crate::Scoring::PerColumn
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Term {
    /// The literal scanned for.
    pub term: String,
    /// CSV column the term came from. `None` for non-CSV
    /// sources; `Some(i)` for the cell at column `i` of a CSV.
    #[serde(default)]
    pub column: Option<u16>,
    /// Optional per-term score override. When `Some`, the
    /// recognizer stamps this score on every match of this term;
    /// when `None`, falls back to the dictionary's [`Scoring`]
    /// policy resolved against [`column`].
    ///
    /// [`Scoring`]: crate::Scoring
    /// [`column`]: Self::column
    #[serde(default)]
    pub score: Option<Confidence>,
}

impl Term {
    /// Construct a term with no column and no per-term score
    /// override. The common path for plain-text sources and
    /// programmatic `From<…>` constructions.
    #[must_use]
    pub fn new(term: impl Into<String>) -> Self {
        Self {
            term: term.into(),
            column: None,
            score: None,
        }
    }

    /// Attach a CSV source-column index, used by the dictionary's
    /// [`Scoring::PerColumn`] to pick a per-column score.
    ///
    /// [`Scoring::PerColumn`]: crate::Scoring::PerColumn
    #[must_use]
    pub fn with_column(mut self, column: u16) -> Self {
        self.column = Some(column);
        self
    }

    /// Set an explicit per-term score, overriding the dictionary's
    /// column-resolved score for this term.
    #[must_use]
    pub fn with_score(mut self, score: Confidence) -> Self {
        self.score = Some(score);
        self
    }
}

impl Terms {
    /// Construct an empty term list.
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Borrow the inner entries.
    #[must_use]
    pub fn entries(&self) -> &[Term] {
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
    pub fn into_inner(self) -> Vec<Term> {
        self.0
    }

    /// Parse terms from plain-text bytes — one term per line.
    /// Each line is trimmed; empty lines and lines starting with
    /// `#` are skipped. Plain-text terms carry no column.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the input is not valid
    /// UTF-8.
    pub fn from_text(bytes: &[u8]) -> Result<Self, Error> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| Error::validation(format!("terms text: {e}"), "nvisy-pattern"))?;
        let entries: Vec<Term> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(Term::new)
            .collect();
        Ok(Self(entries))
    }

    /// Parse terms from CSV bytes. Every non-empty cell across
    /// every row becomes a term, and each term remembers the
    /// (0-based) column index it came from so a [`Dictionary`]
    /// can apply per-column confidence overrides via
    /// [`Scoring::PerColumn`].
    ///
    /// # Errors
    ///
    /// Returns a validation error when the CSV is malformed.
    ///
    /// [`Dictionary`]: crate::Dictionary
    /// [`Scoring::PerColumn`]: crate::Scoring::PerColumn
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
                    let column = u16::try_from(col_idx).unwrap_or(u16::MAX);
                    entries.push(Term::new(trimmed).with_column(column));
                }
            }
        }
        Ok(Self(entries))
    }
}

impl From<Vec<String>> for Terms {
    fn from(terms: Vec<String>) -> Self {
        Self(terms.into_iter().map(Term::new).collect())
    }
}

impl From<&[&str]> for Terms {
    fn from(terms: &[&str]) -> Self {
        Self(terms.iter().copied().map(Term::new).collect())
    }
}

impl<const N: usize> From<[&str; N]> for Terms {
    fn from(terms: [&str; N]) -> Self {
        Self(terms.iter().copied().map(Term::new).collect())
    }
}

impl<const N: usize> From<[String; N]> for Terms {
    fn from(terms: [String; N]) -> Self {
        Self(terms.into_iter().map(Term::new).collect())
    }
}
