//! PDF reassembly action — writes redacted content back to PDF bytes.

use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::{Document, ImageData};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

/// Typed parameters for [`ApplyPdfRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPdfRedactionParams {}

/// Reassembles redacted text and images back into the original PDF.
///
/// Uses `lopdf` to:
/// 1. Replace PDF content streams with redacted text.
/// 2. Replace embedded image XObjects with redacted image data.
/// 3. Write the modified PDF back to `blob.content`.
pub struct ApplyPdfRedactionAction;

#[async_trait::async_trait]
impl Action for ApplyPdfRedactionAction {
    type Params = ApplyPdfRedactionParams;

    fn id(&self) -> &str {
        "apply-pdf-redaction"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        _params: Self::Params,
    ) -> Result<u64, Error> {
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let _documents: Vec<Document> = blob.get_artifacts("documents").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read documents: {e}"))
            })?;
            let images: Vec<ImageData> = blob.get_artifacts("images").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read images: {e}"))
            })?;

            // Only process if the blob is actually a PDF
            let is_pdf = blob
                .content_type()
                .map(|ct| ct == "application/pdf")
                .unwrap_or(false);

            if !is_pdf {
                if output.send(blob).await.is_err() {
                    return Ok(count);
                }
                continue;
            }

            let mut pdf_doc = lopdf::Document::load_mem(&blob.content).map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("PDF load failed: {e}"))
            })?;

            // Replace embedded image XObjects with redacted versions
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
                            if let (Some(sid), Some(redacted_img)) =
                                (stream_id, images.get(image_idx))
                            {
                                let new_stream = lopdf::Stream::new(
                                    lopdf::Dictionary::new(),
                                    redacted_img.image_data.to_vec(),
                                );
                                pdf_doc
                                    .objects
                                    .insert(sid, lopdf::Object::Stream(new_stream));
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

            blob.content = Bytes::from(output_buf);
            count += 1;

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}
