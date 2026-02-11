//! DOCX (Office Open XML) file loader.

use bytes::Bytes;
use serde::Deserialize;
use std::io::Cursor;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::{Document, Element, ElementType, ImageData};
use nvisy_core::error::{Error, ErrorKind};
use super::{Loader, LoaderOutput};

/// Typed parameters for [`DocxLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxLoaderParams {
    /// Whether to extract embedded images.
    #[serde(default = "default_true")]
    pub extract_images: bool,
}

fn default_true() -> bool {
    true
}

/// Extracts text and optionally images from DOCX files.
pub struct DocxLoader;

#[async_trait::async_trait]
impl Loader for DocxLoader {
    type Params = DocxLoaderParams;

    fn id(&self) -> &str {
        "docx"
    }

    fn extensions(&self) -> &[&str] {
        &["docx"]
    }

    fn content_types(&self) -> &[&str] {
        &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"]
    }

    async fn load(
        &self,
        blob: &Blob,
        params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let cursor = Cursor::new(blob.content.to_vec());
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("Failed to open DOCX ZIP: {e}"))
        })?;

        let mut outputs = Vec::new();
        let mut elements = Vec::new();
        let mut full_text = String::new();

        // Parse word/document.xml
        if let Ok(mut entry) = archive.by_name("word/document.xml") {
            let mut xml_content = String::new();
            std::io::Read::read_to_string(&mut entry, &mut xml_content).map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("Failed to read document.xml: {e}"))
            })?;

            let mut reader = quick_xml::Reader::from_str(&xml_content);
            let mut in_text = false;
            let mut in_heading = false;
            let mut current_text = String::new();
            let mut buf = Vec::new();

            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(quick_xml::events::Event::Start(ref e)) => {
                        match e.name().as_ref() {
                            b"w:t" => in_text = true,
                            b"w:pStyle" => {
                                for attr in e.attributes().flatten() {
                                    if attr.key.as_ref() == b"w:val" {
                                        let val = String::from_utf8_lossy(&attr.value);
                                        if val.starts_with("Heading") {
                                            in_heading = true;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(quick_xml::events::Event::End(ref e)) => {
                        match e.name().as_ref() {
                            b"w:t" => in_text = false,
                            b"w:p" => {
                                if !current_text.is_empty() {
                                    let element_type = if in_heading {
                                        ElementType::Title
                                    } else {
                                        ElementType::NarrativeText
                                    };
                                    elements.push(Element::new(element_type, &current_text));
                                    if !full_text.is_empty() {
                                        full_text.push('\n');
                                    }
                                    full_text.push_str(&current_text);
                                    current_text.clear();
                                    in_heading = false;
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(quick_xml::events::Event::Text(ref e)) => {
                        if in_text {
                            let text = e.unescape().unwrap_or_default();
                            current_text.push_str(&text);
                        }
                    }
                    Ok(quick_xml::events::Event::Eof) => break,
                    Err(e) => {
                        tracing::warn!("DOCX XML parse error: {e}");
                        break;
                    }
                    _ => {}
                }
                buf.clear();
            }
        }

        let doc = Document::new(full_text)
            .with_elements(elements)
            .with_source_format("docx");

        outputs.push(LoaderOutput::Document(doc));

        // Extract images from word/media/
        if params.extract_images {
            let media_names: Vec<String> = (0..archive.len())
                .filter_map(|i| {
                    let entry = archive.by_index(i).ok()?;
                    let name = entry.name().to_string();
                    if name.starts_with("word/media/") {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect();

            for name in media_names {
                if let Ok(mut entry) = archive.by_name(&name) {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut entry, &mut buf).ok();
                    if !buf.is_empty() {
                        let mime = infer::get(&buf)
                            .map(|t| t.mime_type().to_string())
                            .unwrap_or_else(|| "image/png".to_string());
                        let img = ImageData::new(Bytes::from(buf), mime)
                            .with_source_path(&name);
                        outputs.push(LoaderOutput::Image(img));
                    }
                }
            }
        }

        Ok(outputs)
    }
}
