//! Element type ontology and category classification.

use serde::{Deserialize, Serialize};

/// Broad grouping of element types.
///
/// Every [`ElementType`] belongs to exactly one category, providing
/// a coarse filter for pipeline actions that only operate on certain
/// kinds of content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ElementCategory {
    /// Narrative text, headings, list items, captions, and addresses.
    Text,
    /// Tabular data.
    Table,
    /// Images and other media content.
    Media,
    /// Source code fragments.
    Code,
    /// Mathematical formulae.
    Math,
    /// Form elements such as checkboxes and key-value fields.
    Form,
    /// Layout markers like page breaks and page numbers.
    Layout,
    /// Email message content.
    Email,
}

/// Specific structural element type extracted from a document.
///
/// Each variant maps to a single [`ElementCategory`] via
/// [`ElementType::category`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum ElementType {
    // -- Text --

    /// A document title or section heading.
    Title,
    /// A block of narrative prose.
    NarrativeText,
    /// An item within a bulleted or numbered list.
    ListItem,
    /// A page or section header.
    Header,
    /// A page or section footer.
    Footer,
    /// Caption text associated with a figure.
    FigureCaption,
    /// A physical or mailing address.
    Address,
    /// Text that does not fit any other text category.
    UncategorizedText,

    // -- Table --

    /// A data table with rows and columns.
    Table,

    // -- Media --

    /// An embedded image.
    Image,

    // -- Code --

    /// A source code snippet or block.
    CodeSnippet,

    // -- Math --

    /// A mathematical formula or equation.
    Formula,

    // -- Form --

    /// A checkbox form control.
    Checkbox,
    /// A set of key-value pairs extracted from a form.
    FormKeysValues,

    // -- Layout --

    /// A page break marker.
    PageBreak,
    /// A page number indicator.
    PageNumber,

    // -- Email --

    /// An email message body and headers.
    EmailMessage,
}

impl ElementType {
    /// Return the category this element type belongs to.
    pub fn category(&self) -> ElementCategory {
        match self {
            Self::Title
            | Self::NarrativeText
            | Self::ListItem
            | Self::Header
            | Self::Footer
            | Self::FigureCaption
            | Self::Address
            | Self::UncategorizedText => ElementCategory::Text,
            Self::Table => ElementCategory::Table,
            Self::Image => ElementCategory::Media,
            Self::CodeSnippet => ElementCategory::Code,
            Self::Formula => ElementCategory::Math,
            Self::Checkbox | Self::FormKeysValues => ElementCategory::Form,
            Self::PageBreak | Self::PageNumber => ElementCategory::Layout,
            Self::EmailMessage => ElementCategory::Email,
        }
    }
}

/// Parse an element type string and return its category.
///
/// Returns `None` if the string does not match any known [`ElementType`].
pub fn category_of(type_str: &str) -> Option<ElementCategory> {
    let et: ElementType =
        serde_json::from_value(serde_json::Value::String(type_str.to_string())).ok()?;
    Some(et.category())
}
