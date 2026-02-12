//! HTML file loader using the `scraper` crate.

use serde::Deserialize;

use nvisy_core::io::ContentData;
use nvisy_core::error::{Error, ErrorKind};

use crate::document::Document;
use crate::element::{Element, ElementType};
use crate::handler::{HtmlHandler, FormatHandler, TextLoader};

/// Typed parameters for [`HtmlLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlLoaderParams {}

/// Extracts text and structural elements from HTML documents.
pub struct HtmlLoader;

impl Clone for HtmlLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl TextLoader for HtmlLoader {
    type Params = HtmlLoaderParams;

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let html_str = String::from_utf8(content.to_bytes().to_vec()).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("HTML is not valid UTF-8: {e}"))
        })?;

        let document = scraper::Html::parse_document(&html_str);
        let mut elements = Vec::new();
        let mut full_text = String::new();

        // Map HTML tags to element types
        let tag_mappings: &[(&str, ElementType)] = &[
            ("h1", ElementType::Title),
            ("h2", ElementType::Title),
            ("h3", ElementType::Title),
            ("h4", ElementType::Title),
            ("h5", ElementType::Title),
            ("h6", ElementType::Title),
            ("p", ElementType::NarrativeText),
            ("li", ElementType::ListItem),
            ("table", ElementType::Table),
            ("pre", ElementType::CodeSnippet),
            ("code", ElementType::CodeSnippet),
            ("address", ElementType::Address),
            ("header", ElementType::Header),
            ("footer", ElementType::Footer),
            ("figcaption", ElementType::FigureCaption),
        ];

        for (tag, element_type) in tag_mappings {
            let selector = scraper::Selector::parse(tag).unwrap();
            for element in document.select(&selector) {
                let text: String = element.text().collect::<Vec<_>>().join(" ");
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let mut el = Element::new(*element_type, trimmed);
                // Set heading level for h1-h6
                if tag.starts_with('h') && tag.len() == 2 {
                    if let Some(level) = tag[1..].parse::<u32>().ok() {
                        el = el.with_level(level);
                    }
                }
                if !full_text.is_empty() {
                    full_text.push('\n');
                }
                full_text.push_str(trimmed);
                elements.push(el);
            }
        }

        // If no structured elements found, extract all body text
        if elements.is_empty() {
            let body_selector = scraper::Selector::parse("body").unwrap();
            if let Some(body) = document.select(&body_selector).next() {
                full_text = body.text().collect::<Vec<_>>().join(" ");
                let trimmed = full_text.trim().to_string();
                full_text = trimmed;
            }
        }

        let doc = Document::new(HtmlHandler)
            .with_text(full_text)
            .with_elements(elements)
            .into_format();

        Ok(vec![doc])
    }
}

impl crate::handler::Handler for HtmlLoader {
    fn id(&self) -> &str { HtmlHandler.id() }
    fn extensions(&self) -> &[&str] { HtmlHandler.extensions() }
    fn content_types(&self) -> &[&str] { HtmlHandler.content_types() }
}
