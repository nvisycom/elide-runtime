//! Plain-text dictionary (one entry per line).

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
    /// The dictionary name is derived from the file stem of `path`
    /// (e.g. `"assets/nationalities.txt"` → `"nationalities"`).
    pub fn new(path: impl AsRef<Path>, text: &str) -> Self {
        let name = path
            .as_ref()
            .file_stem()
            .expect("dictionary path has no file stem")
            .to_string_lossy()
            .into_owned();

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
        let dict = TxtDictionary::new("test.txt", "alpha\n  beta \n\ngamma\n");
        assert_eq!(dict.name(), "test");
        assert_eq!(dict.entries(), &["alpha", "beta", "gamma"]);
    }

    #[test]
    fn derives_name_from_path() {
        let dict = TxtDictionary::new("assets/dictionaries/nationalities.txt", "a\n");
        assert_eq!(dict.name(), "nationalities");
    }
}
