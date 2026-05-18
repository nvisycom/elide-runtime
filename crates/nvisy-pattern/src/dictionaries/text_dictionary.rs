//! Plain-text dictionary: one entry per line.

use std::path::Path;
use std::{fs, io};

use super::{Dictionary, DictionaryTerm};

/// A dictionary parsed from a plain-text file (one entry per line).
#[derive(Debug)]
pub struct TxtDictionary {
    name: String,
    terms: Vec<DictionaryTerm>,
}

impl TxtDictionary {
    /// Parse a plain-text dictionary.
    ///
    /// `name` identifies this dictionary (e.g. `"nationalities"`).
    /// `text` is the file content with one entry per line.
    pub fn new(name: impl Into<String>, text: &str) -> Self {
        let name = name.into();

        let terms = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| DictionaryTerm {
                value: l.to_owned(),
                column: None,
            })
            .collect();

        Self { name, terms }
    }

    /// Load a plain-text dictionary from a file path.
    ///
    /// The dictionary name is derived from the file stem.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if the file cannot be read.
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let text = fs::read_to_string(path)?;
        Ok(Self::new(name, &text))
    }
}

impl Dictionary for TxtDictionary {
    fn name(&self) -> &str {
        &self.name
    }

    fn terms(&self) -> &[DictionaryTerm] {
        &self.terms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines() {
        let dict = TxtDictionary::new("test", "alpha\n  beta \n\ngamma\n");
        assert_eq!(dict.name(), "test");

        let values: Vec<&str> = dict.terms().iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, &["alpha", "beta", "gamma"]);

        assert!(dict.terms().iter().all(|t| t.column.is_none()));
    }
}
