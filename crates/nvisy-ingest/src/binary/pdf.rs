//! PDF file loader using `pdf-extract` and `lopdf`.

use bytes::Bytes;
use serde::Deserialize;

use nvisy_core::io::ContentData;
use nvisy_core::error::{Error, ErrorKind};

use crate::document::Document;
use crate::handler::{PdfHandler, ImageHandler, FormatHandler, BinaryLoader};

/// Typed parameters for [`PdfLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfLoaderParams {
    /// Whether to extract embedded images from the PDF.
    #[serde(default = "default_true")]
    pub extract_images: bool,
    /// Maximum number of pages to process. `None` means all pages.
    #[serde(default)]
    pub max_pages: Option<u32>,
}

fn default_true() -> bool {
    true
}

/// Extracts text and optionally images from PDF files.
pub struct PdfLoader;

impl Clone for PdfLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl BinaryLoader for PdfLoader {
    type Params = PdfLoaderParams;

    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let bytes = content.to_bytes().to_vec();
        let mut documents = Vec::new();

        // Extract text
        let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PDF text extraction failed: {e}"))
        })?;

        let lop_doc = lopdf::Document::load_mem(&bytes).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PDF parsing failed: {e}"))
        })?;

        let page_count = lop_doc.get_pages().len() as u32;

        let mut doc = Document::new(PdfHandler)
            .with_text(text)
            .with_page_count(page_count);
        doc.source.set_parent_id(Some(content.content_source.as_uuid()));
        documents.push(doc.into_format());

        // Extract embedded images
        if params.extract_images {
            let max_pages = params.max_pages.unwrap_or(page_count);
            for (page_num, page_id) in lop_doc.get_pages() {
                if page_num > max_pages {
                    break;
                }

                let (resources_opt, _) = match lop_doc.get_page_resources(page_id) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let resources = match resources_opt {
                    Some(res) => res,
                    None => continue,
                };

                let xobject_obj = match resources.get(b"XObject") {
                    Ok(obj) => obj,
                    Err(_) => continue,
                };

                let xobjects = match lop_doc.dereference(xobject_obj) {
                    Ok((_, lopdf::Object::Dictionary(dict))) => dict.clone(),
                    _ => continue,
                };

                for (_name, obj_ref) in xobjects.iter() {
                    let stream = match lop_doc.dereference(obj_ref) {
                        Ok((_, lopdf::Object::Stream(s))) => s.clone(),
                        _ => continue,
                    };

                    let is_image = stream
                        .dict
                        .get(b"Subtype")
                        .ok()
                        .and_then(|s| {
                            if let lopdf::Object::Name(n) = s {
                                Some(n.as_slice() == b"Image")
                            } else {
                                None
                            }
                        })
                        .unwrap_or(false);

                    if !is_image {
                        continue;
                    }

                    let image_bytes = stream.content.clone();
                    if image_bytes.is_empty() {
                        continue;
                    }

                    let width = stream
                        .dict
                        .get(b"Width")
                        .ok()
                        .and_then(|w| {
                            if let lopdf::Object::Integer(i) = w {
                                Some(*i as u32)
                            } else {
                                None
                            }
                        });

                    let height = stream
                        .dict
                        .get(b"Height")
                        .ok()
                        .and_then(|h| {
                            if let lopdf::Object::Integer(i) = h {
                                Some(*i as u32)
                            } else {
                                None
                            }
                        });

                    let mut img = Document::new(ImageHandler)
                        .with_data(Bytes::from(image_bytes), "image/png")
                        .with_page_number(page_num);

                    if let (Some(w), Some(h)) = (width, height) {
                        img = img.with_dimensions(w, h);
                    }

                    img.source.set_parent_id(Some(content.content_source.as_uuid()));
                    documents.push(img.into_format());
                }
            }
        }

        Ok(documents)
    }
}

impl crate::handler::Handler for PdfLoader {
    fn id(&self) -> &str { PdfHandler.id() }
    fn extensions(&self) -> &[&str] { PdfHandler.extensions() }
    fn content_types(&self) -> &[&str] { PdfHandler.content_types() }
}
