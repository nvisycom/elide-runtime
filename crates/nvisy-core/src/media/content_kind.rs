//! Content type classification for different categories of data
//!
//! This module provides the [`ContentKind`] enum for classifying content
//! into broad categories. Extension-to-kind mapping is handled by the
//! engine's format registry.

use derive_more::IsVariant;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString};

/// Content type classification for different categories of data
///
/// This enum represents high-level content categories without knowledge
/// of specific file extensions or MIME types. The engine's format registry
/// handles the mapping from extensions/MIME types to content kinds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(AsRefStr, Display, EnumString, EnumIter, IsVariant)]
#[derive(Serialize, Deserialize)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    /// Plain text content
    Text,
    /// Document files (PDF, Word, etc.)
    Document,
    /// Spreadsheet files (Excel, CSV, etc.)
    Spreadsheet,
    /// Image files
    Image,
    /// Unknown or unsupported content type
    #[default]
    Unknown,
}
