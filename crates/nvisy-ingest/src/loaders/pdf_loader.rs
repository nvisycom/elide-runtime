//! PDF file loader using `pdf-extract` and `lopdf`.

use bytes::Bytes;
use serde::Deserialize;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::{Document, ImageData};
use nvisy_core::error::{Error, ErrorKind};
use super::{Loader, LoaderOutput};

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

#[async_trait::async_trait]
impl Loader for PdfLoader {
    type Params = PdfLoaderParams;

    fn id(&self) -> &str {
        "pdf"
    }

    fn extensions(&self) -> &[&str] {
        &["pdf"]
    }

    fn content_types(&self) -> &[&str] {
        &["application/pdf"]
    }

    async fn load(
        &self,
        blob: &Blob,
        params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let bytes = blob.content.to_vec();
        let mut outputs = Vec::new();

        // Extract text
        let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PDF text extraction failed: {e}"))
        })?;

        let lop_doc = lopdf::Document::load_mem(&bytes).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PDF parsing failed: {e}"))
        })?;

        let page_count = lop_doc.get_pages().len() as u32;

        let doc = Document::new(text)
            .with_source_format("pdf")
            .with_page_count(page_count);

        outputs.push(LoaderOutput::Document(doc));

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

                    let mut img = ImageData::new(
                        Bytes::from(image_bytes),
                        "image/png",
                    )
                    .with_page_number(page_num);

                    if let (Some(w), Some(h)) = (width, height) {
                        img = img.with_dimensions(w, h);
                    }

                    outputs.push(LoaderOutput::Image(img));
                }
            }
        }

        Ok(outputs)
    }
}
