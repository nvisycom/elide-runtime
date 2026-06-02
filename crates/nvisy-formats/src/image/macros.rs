//! [`impl_image_handler!`]: shared macro for single-image handler
//! structs (PNG, JPEG, TIFF — anything backed by one
//! `image::DynamicImage` + a `ContentSource`).
//!
//! Generates the `Handler` impl, the `Handle<Image>` impl that calls
//! the local [`crate::image::redact`] helper, and the inherent
//! `new` / `with_source` / `image` accessors.

/// Implement [`Handler`] + [`Handle<Image>`] + inherent methods for
/// an image handler struct that holds a single `DynamicImage` and a
/// `ContentSource`.
///
/// Crate-internal paths are fully qualified via `$crate::…` so the
/// macro stays self-contained inside `nvisy-formats`.
///
/// [`Handler`]: nvisy_codec::handler::Handler
/// [`Handle<Image>`]: nvisy_codec::core::Handle
#[macro_export]
macro_rules! impl_image_handler {
    ($handler:ident, $doc_type:expr, $fmt:expr, $origin:literal, $encode_name:literal) => {
        impl ::nvisy_codec::handler::Handler for $handler {
            fn document_type(&self) -> ::nvisy_core::content::DocumentType {
                $doc_type
            }

            fn source(&self) -> ::nvisy_core::content::ContentSource {
                self.source
            }

            #[::tracing::instrument(name = $encode_name, skip_all, fields(output_bytes))]
            fn encode(
                &self,
            ) -> ::std::result::Result<::nvisy_core::content::ContentData, ::nvisy_core::Error>
            {
                use ::std::io::Cursor;

                let mut buf = Cursor::new(Vec::new());
                self.image.write_to(&mut buf, $fmt).map_err(|e| {
                    ::nvisy_core::Error::validation(format!("encode failed: {e}"), $origin)
                })?;
                let out = buf.into_inner();
                ::tracing::Span::current().record("output_bytes", out.len());
                let source = ::nvisy_core::content::ContentSource::new().with_parent(&self.source);
                Ok(::nvisy_core::content::ContentData::new(source, out.into()))
            }
        }

        #[::async_trait::async_trait]
        impl ::nvisy_codec::core::Handle<::nvisy_ontology::modality::Image> for $handler {
            fn locations(
                &self,
            ) -> ::nvisy_codec::core::LocationStream<'_, ::nvisy_ontology::modality::Image> {
                use ::std::iter;

                let (w, h) = (self.image.width(), self.image.height());
                let location = ::nvisy_ontology::modality::Image {
                    bounding_box: ::nvisy_ontology::primitive::BoundingBox {
                        x: 0.0,
                        y: 0.0,
                        width: w as f64,
                        height: h as f64,
                    },
                    polygon: None,
                    image_id: None,
                    page_number: None,
                };
                ::nvisy_codec::core::LocationStream::new(::futures::stream::iter(iter::once(
                    ::nvisy_codec::core::Located::new(self.source, location),
                )))
            }

            async fn read(
                &self,
                location: &::nvisy_ontology::modality::Image,
            ) -> Option<::nvisy_codec::handler::ImageData> {
                let bb = &location.bounding_box;
                let x = bb.x.max(0.0) as u32;
                let y = bb.y.max(0.0) as u32;
                let w = (bb.width as u32).min(self.image.width().saturating_sub(x));
                let h = (bb.height as u32).min(self.image.height().saturating_sub(y));
                if w == 0 || h == 0 {
                    return None;
                }
                let cropped = self.image.crop_imm(x, y, w, h);
                Some(::nvisy_codec::handler::ImageData::from(cropped))
            }

            async fn redact_at(
                &mut self,
                location: &::nvisy_ontology::modality::Image,
                redaction: ::nvisy_codec::handler::ImageRedaction,
            ) -> ::std::result::Result<(), ::nvisy_core::Error> {
                $crate::image::redact::apply(&mut self.image, &redaction, location.bounding_box);
                Ok(())
            }
        }

        impl $handler {
            /// Create a handler from an already-decoded image.
            pub fn new(image: ::image::DynamicImage) -> Self {
                Self {
                    source: ::nvisy_core::content::ContentSource::new(),
                    image,
                }
            }

            /// Set the content source for lineage tracking.
            pub fn with_source(mut self, source: ::nvisy_core::content::ContentSource) -> Self {
                self.source = source;
                self
            }

            /// Reference to the decoded image.
            pub fn image(&self) -> &::image::DynamicImage {
                &self.image
            }
        }
    };
}
