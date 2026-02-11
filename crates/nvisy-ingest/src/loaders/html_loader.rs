//! HTML file loader using the `scraper` crate.

use serde::Deserialize;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::{Document, Element, ElementType};
use nvisy_core::error::{Error, ErrorKind};
use super::{Loader, LoaderOutput};

/// Typed parameters for [`HtmlLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlLoaderParams {}

/// Extracts text and structural elements from HTML documents.
pub struct HtmlLoader;

#[async_trait::async_trait]
impl Loader for HtmlLoader {
    type Params = HtmlLoaderParams;

    fn id(&self) -> &str {
        "html"
    }

    fn extensions(&self) -> &[&str] {
        &["html", "htm"]
    }

    fn content_types(&self) -> &[&str] {
        &["text/html"]
    }

    async fn load(
        &self,
        blob: &Blob,
        _params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let html_str = String::from_utf8(blob.content.to_vec()).map_err(|e| {
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

        let doc = Document::new(full_text)
            .with_elements(elements)
            .with_source_format("html");

        Ok(vec![LoaderOutput::Document(doc)])
    }
}
