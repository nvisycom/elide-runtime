//! [`impl_image_handler!`]: shared macro for image handler structs.

/// Implement [`Handler`] + [`ImageHandler`] + inherent methods for an
/// image handler struct that holds a single `DynamicImage`.
macro_rules! impl_image_handler {
    ($handler:ident, $doc_type:expr, $fmt:expr, $origin:literal, $encode_name:literal) => {
        impl crate::handler::Handler for $handler {
            fn document_type(&self) -> nvisy_core::media::DocumentType {
                $doc_type
            }

            fn source(&self) -> nvisy_core::content::ContentSource {
                self.source
            }

            #[tracing::instrument(name = $encode_name, skip_all, fields(output_bytes))]
            fn encode(&self) -> Result<nvisy_core::content::ContentData, nvisy_core::Error> {
                let mut buf = std::io::Cursor::new(Vec::new());
                self.image.write_to(&mut buf, $fmt).map_err(|e| {
                    nvisy_core::Error::validation(format!("encode failed: {e}"), $origin)
                })?;
                let out = buf.into_inner();
                tracing::Span::current().record("output_bytes", out.len());
                let source = nvisy_core::content::ContentSource::new().with_parent(&self.source);
                Ok(nvisy_core::content::ContentData::new(source, out.into()))
            }
        }

        #[async_trait::async_trait]
        impl crate::handler::ImageHandler for $handler {
            async fn image_spans(
                &self,
            ) -> crate::document::SpanStream<
                '_,
                nvisy_ontology::entity::ImageLocation,
                crate::handler::ImageData,
            > {
                let (w, h) = (self.image.width(), self.image.height());
                let location = nvisy_ontology::entity::ImageLocation {
                    bounding_box: nvisy_ontology::math::BoundingBox {
                        x: 0.0,
                        y: 0.0,
                        width: w as f64,
                        height: h as f64,
                    },
                    value: None,
                    image_id: None,
                    page_number: None,
                };
                crate::document::SpanStream::new(futures::stream::iter(std::iter::once(
                    crate::document::Span::new(
                        location,
                        crate::handler::ImageData::from(self.image.clone()),
                    ),
                )))
            }

            async fn edit_images(
                &mut self,
                edits: crate::document::SpanStream<
                    '_,
                    nvisy_ontology::entity::ImageLocation,
                    crate::handler::ImageData,
                >,
            ) -> Result<(), nvisy_core::Error> {
                use futures::StreamExt;
                let edits: Vec<_> = edits.collect().await;
                if let Some(edit) = edits.into_iter().next() {
                    self.image = edit.data.into_inner();
                }
                Ok(())
            }

            async fn value_at(
                &self,
                location: &nvisy_ontology::entity::ImageLocation,
            ) -> Option<crate::handler::ImageData> {
                let bb = &location.bounding_box;
                let x = bb.x.max(0.0) as u32;
                let y = bb.y.max(0.0) as u32;
                let w = (bb.width as u32).min(self.image.width().saturating_sub(x));
                let h = (bb.height as u32).min(self.image.height().saturating_sub(y));
                if w == 0 || h == 0 {
                    return None;
                }
                let cropped = self.image.crop_imm(x, y, w, h);
                Some(crate::handler::ImageData::from(cropped))
            }
        }

        impl $handler {
            /// Create a handler from an already-decoded image.
            pub fn new(image: image::DynamicImage) -> Self {
                Self {
                    source: nvisy_core::content::ContentSource::new(),
                    image,
                }
            }

            /// Set the content source for lineage tracking.
            pub fn with_source(mut self, source: nvisy_core::content::ContentSource) -> Self {
                self.source = source;
                self
            }

            /// Reference to the decoded image.
            pub fn image(&self) -> &image::DynamicImage {
                &self.image
            }
        }
    };
}

pub(crate) use impl_image_handler;
