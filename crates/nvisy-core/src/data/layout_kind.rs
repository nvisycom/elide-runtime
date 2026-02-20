//! Document-layout region kinds.
//!
//! [`LayoutKind`] identifies structural regions within a document that
//! are not themselves sensitive data but contain or frame sensitive
//! content.

use serde::{Deserialize, Serialize};

/// Kind of structural / layout region within a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum LayoutKind {
    // ── Tabular:
    /// Tabular data region.
    Table,
    /// Row within tabular data.
    Row,
    /// Named column within tabular data.
    Column,
    /// Individual cell within tabular data.
    Cell,

    // ── Text:
    /// Section heading or title.
    Heading,
    /// Paragraph of body text.
    Paragraph,
    /// Ordered or unordered list.
    List,
    /// Individual list item.
    ListItem,
    /// Block quotation.
    BlockQuote,
    /// Code block or preformatted text region.
    CodeBlock,

    // ── Media:
    /// Embedded or inline image.
    Image,
    /// Figure with optional caption.
    Figure,

    // ── Document structure:
    /// Page header region.
    Header,
    /// Page footer region.
    Footer,
    /// Footnote or endnote.
    Footnote,
    /// Page break boundary.
    PageBreak,
    /// Form field (text input, checkbox, dropdown, etc.).
    FormField,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn display_snake_case() {
        assert_eq!(LayoutKind::Table.to_string(), "table");
        assert_eq!(LayoutKind::Column.to_string(), "column");
        assert_eq!(LayoutKind::Image.to_string(), "image");
        assert_eq!(LayoutKind::Heading.to_string(), "heading");
        assert_eq!(LayoutKind::CodeBlock.to_string(), "code_block");
        assert_eq!(LayoutKind::FormField.to_string(), "form_field");
    }

    #[test]
    fn parse_roundtrip() {
        let kind = LayoutKind::from_str("table").unwrap();
        assert_eq!(kind, LayoutKind::Table);

        let kind = LayoutKind::from_str("block_quote").unwrap();
        assert_eq!(kind, LayoutKind::BlockQuote);
    }

    #[test]
    fn serde_roundtrip() {
        let kind = LayoutKind::Footnote;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"footnote\"");
        let back: LayoutKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }

    #[test]
    fn layout_kind_is_copy() {
        let a = LayoutKind::Table;
        let b = a;
        assert_eq!(a, b);
    }
}
