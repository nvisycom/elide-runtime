use serde::{Deserialize, Serialize};

/// Element category — broad grouping of element types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ElementCategory {
    Text,
    Table,
    Media,
    Code,
    Math,
    Form,
    Layout,
    Email,
}

/// All element types across all categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum ElementType {
    // Text
    Title,
    NarrativeText,
    ListItem,
    Header,
    Footer,
    FigureCaption,
    Address,
    UncategorizedText,
    // Table
    Table,
    // Media
    Image,
    // Code
    CodeSnippet,
    // Math
    Formula,
    // Form
    Checkbox,
    FormKeysValues,
    // Layout
    PageBreak,
    PageNumber,
    // Email
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

/// Return the category for a given element type string.
pub fn category_of(type_str: &str) -> Option<ElementCategory> {
    let et: ElementType =
        serde_json::from_value(serde_json::Value::String(type_str.to_string())).ok()?;
    Some(et.category())
}
