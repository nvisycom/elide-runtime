//! Plain-text dictionary: one entry per line.

use super::{Dictionary, DictionaryMetadata, DictionaryTerm};

/// A dictionary parsed from a plain-text file (one entry per line).
#[derive(Debug)]
pub struct TxtDictionary {
    name: String,
    terms: Vec<DictionaryTerm>,
    metadata: DictionaryMetadata,
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

        Self {
            name,
            terms,
            metadata: DictionaryMetadata::default(),
        }
    }

    /// Attach metadata loaded from a sidecar file.
    pub fn with_metadata(mut self, metadata: DictionaryMetadata) -> Self {
        self.metadata = metadata;
        self
    }

}

impl Dictionary for TxtDictionary {
    fn name(&self) -> &str {
        &self.name
    }

    fn terms(&self) -> &[DictionaryTerm] {
        &self.terms
    }

    fn metadata(&self) -> &DictionaryMetadata {
        &self.metadata
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
