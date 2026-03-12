//! Plain-text dictionary: one matchable entry per line.

use std::path::Path;

use super::Dictionary;

/// A dictionary parsed from a plain-text file (one entry per line).
#[derive(Debug, Clone)]
pub struct TxtDictionary {
    name: String,
    entries: Vec<String>,
}

impl TxtDictionary {
    /// Parse a plain-text dictionary.
    ///
    /// `name` identifies this dictionary (e.g. `"nationalities"`).
    /// `text` is the file content with one entry per line.
    pub fn new(name: impl Into<String>, text: &str) -> Self {
        let name = name.into();

        let entries = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();

        Self { name, entries }
    }

    /// Load a plain-text dictionary from a file path.
    ///
    /// The dictionary name is derived from the file stem.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if the file cannot be read.
    pub fn from_path(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let text = std::fs::read_to_string(path)?;
        Ok(Self::new(name, &text))
    }
}

impl Dictionary for TxtDictionary {
    fn name(&self) -> &str {
        &self.name
    }

    fn entries(&self) -> &[String] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines() {
        let dict = TxtDictionary::new("test", "alpha\n  beta \n\ngamma\n");
        assert_eq!(dict.name(), "test");
        assert_eq!(dict.entries(), &["alpha", "beta", "gamma"]);
    }
}
