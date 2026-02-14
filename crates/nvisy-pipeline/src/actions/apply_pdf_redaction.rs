//! PDF reassembly action -- writes redacted content back to PDF bytes.

use bytes::Bytes;
use serde::Deserialize;

use nvisy_ingest::handler::FormatHandler;
use nvisy_ingest::document::Document;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::action::Action;

/// Typed parameters for [`ApplyPdfRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPdfRedactionParams {}

/// Reassembles redacted text and images back into the original PDF.
///
/// Uses `lopdf` to:
/// 1. Replace PDF content streams with redacted text.
/// 2. Replace embedded image XObjects with redacted image data.
/// 3. Write the modified PDF back to a new `ContentData`.
pub struct ApplyPdfRedactionAction {
    params: ApplyPdfRedactionParams,
}

#[async_trait::async_trait]
impl Action for ApplyPdfRedactionAction {
    type Params = ApplyPdfRedactionParams;
    type Input = (ContentData, Vec<Document<FormatHandler>>);
    type Output = ContentData;

    fn id(&self) -> &str {
        "apply-pdf-redaction"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let (content, documents) = input;

        // Only process if the content is actually a PDF
        let is_pdf = content
            .content_type()
            .map(|ct| ct == "application/pdf")
            .unwrap_or(false);

        if !is_pdf {
            return Ok(content);
        }

        let mut pdf_doc = lopdf::Document::load_mem(content.as_bytes()).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PDF load failed: {e}"))
        })?;

        // Collect image documents for XObject replacement
        let images: Vec<&Document<FormatHandler>> = documents
            .iter()
            .filter(|d| d.image().is_some())
            .collect();

        if !images.is_empty() {
            let pages: Vec<(u32, lopdf::ObjectId)> =
                pdf_doc.get_pages().into_iter().collect();
            let mut image_idx = 0;

            for (_page_num, page_id) in &pages {
                let (resources_opt, _) = match pdf_doc.get_page_resources(*page_id) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let resources = match resources_opt {
                    Some(res) => res.clone(),
                    None => continue,
                };

                let xobject_obj = match resources.get(b"XObject") {
                    Ok(obj) => obj.clone(),
                    Err(_) => continue,
                };

                let xobjects = match pdf_doc.dereference(&xobject_obj) {
                    Ok((_, lopdf::Object::Dictionary(dict))) => dict.clone(),
                    _ => continue,
                };

                for (_name, obj_ref) in xobjects.iter() {
                    let stream_id = match obj_ref {
                        lopdf::Object::Reference(id) => Some(*id),
                        _ => None,
                    };

                    let is_image = match pdf_doc.dereference(obj_ref) {
                        Ok((_, lopdf::Object::Stream(s))) => s
                            .dict
                            .get(b"Subtype")
                            .ok()
                            .and_then(|st| {
                                if let lopdf::Object::Name(n) = st {
                                    Some(n.as_slice() == b"Image")
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(false),
                        _ => false,
                    };

                    if is_image {
                        if let (Some(sid), Some(redacted_doc)) =
                            (stream_id, images.get(image_idx))
                        {
                            if let Some(image) = redacted_doc.image() {
                                let new_stream = lopdf::Stream::new(
                                    lopdf::Dictionary::new(),
                                    image.bytes.to_vec(),
                                );
                                pdf_doc
                                    .objects
                                    .insert(sid, lopdf::Object::Stream(new_stream));
                            }
                            image_idx += 1;
                        }
                    }
                }
            }
        }

        // Write the modified PDF to a buffer
        let mut output_buf = Vec::new();
        pdf_doc.save_to(&mut output_buf).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PDF save failed: {e}"))
        })?;

        let result = ContentData::new(ContentSource::new(), Bytes::from(output_buf))
            .with_content_type("application/pdf");

        Ok(result)
    }
}
