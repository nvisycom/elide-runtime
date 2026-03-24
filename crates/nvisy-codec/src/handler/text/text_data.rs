//! [`TextData`]: opaque wrapper around extracted text content.

use derive_more::{AsRef, Display, From};
use hipstr::HipStr;

/// Opaque wrapper around a text span's content.
///
/// Mirrors [`ImageData`](crate::handler::ImageData) for text-bearing
/// handlers, providing a consistent type boundary at the `Handler`
/// trait level.
///
/// Internally backed by [`HipStr`] for cheap cloning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Display, From, AsRef)]
#[as_ref(forward)]
pub struct TextData(HipStr<'static>);

impl TextData {
    /// View the inner string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Consume the wrapper and return the inner `String`.
    pub fn into_inner(self) -> String {
        self.0.into()
    }
}

impl From<String> for TextData {
    fn from(s: String) -> Self {
        Self(HipStr::from(s))
    }
}

impl From<&str> for TextData {
    fn from(s: &str) -> Self {
        Self(HipStr::from(s))
    }
}

impl PartialEq<&str> for TextData {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
    }
}
