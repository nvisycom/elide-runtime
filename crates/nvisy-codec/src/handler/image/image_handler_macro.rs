//! [`impl_image_handler!`]: shared macro for image handler structs.

/// Implement [`Handler`] + [`ImageHandler`] + inherent methods for an
/// image handler struct that holds a single `DynamicImage`.
macro_rules! impl_image_handler {
    ($handler:ident, $doc_type:expr, $fmt:expr, $origin:literal, $encode_name:literal) => {
        impl crate::handler::Handler for $handler {
            fn document_type(&self) -> nvisy_core::fs::DocumentType {
                $doc_type
            }

            #[tracing::instrument(name = $encode_name, skip_all, fields(output_bytes))]
            fn encode(&self) -> Result<nvisy_core::io::ContentData, nvisy_core::Error> {
                let mut buf = std::io::Cursor::new(Vec::new());
                self.image.write_to(&mut buf, $fmt).map_err(|e| {
                    nvisy_core::Error::validation(format!("encode failed: {e}"), $origin)
                })?;
                let out = buf.into_inner();
                tracing::Span::current().record("output_bytes", out.len());
                let source = nvisy_core::path::ContentSource::new().with_parent(&self.source);
                Ok(nvisy_core::io::ContentData::new(source, out.into()))
            }
        }

        #[async_trait::async_trait]
        impl crate::handler::ImageHandler for $handler {
            type ImageId = ();

            async fn image_spans(
                &self,
            ) -> crate::handler::SpanStream<'_, (), crate::handler::ImageData> {
                crate::handler::SpanStream::new(futures::stream::iter(std::iter::once(
                    crate::handler::Span::new(
                        (),
                        crate::handler::ImageData::from(self.image.clone()),
                    ),
                )))
            }

            async fn edit_images(
                &mut self,
                edits: crate::handler::SpanEditStream<'_, (), crate::handler::ImageData>,
            ) -> Result<(), nvisy_core::Error> {
                use futures::StreamExt;
                let edits: Vec<_> = edits.collect().await;
                if let Some(edit) = edits.into_iter().next() {
                    self.image = edit.data.into_inner();
                }
                Ok(())
            }
        }

        impl $handler {
            /// Create a handler from an already-decoded image.
            pub fn new(image: image::DynamicImage) -> Self {
                Self {
                    source: nvisy_core::path::ContentSource::new(),
                    image,
                }
            }

            /// Set the content source for lineage tracking.
            pub fn with_source(mut self, source: nvisy_core::path::ContentSource) -> Self {
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
