//! [`FormatId`]: stable identifier for a registered codec format.
//!
//! Open string namespace — downstream crates ship their own formats
//! by registering a [`Format`] descriptor with a unique
//! [`FormatId`].
//!
//! Convention: dot-separated namespace. Built-in formats use the
//! `nvisy.` prefix (e.g. `"nvisy.text.txt"`, `"nvisy.rich.pdf"`).
//! Third-party formats use their own namespace
//! (e.g. `"acme.parquet.v2"`).
//!
//! [`Format`]: crate::Format

use std::borrow::Cow;
use std::fmt;

/// Stable identifier for a registered codec format.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FormatId(Cow<'static, str>);

impl FormatId {
    /// Construct from a static string literal — no allocation.
    pub const fn from_static(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// Construct from an owned [`String`].
    pub fn from_owned(id: String) -> Self {
        Self(Cow::Owned(id))
    }

    /// Borrow as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FormatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for FormatId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
