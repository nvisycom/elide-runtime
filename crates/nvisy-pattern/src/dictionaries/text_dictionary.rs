//! Plain-text dictionary: one matchable entry per line.

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
