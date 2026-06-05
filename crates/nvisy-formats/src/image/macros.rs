//! [`impl_image_handler!`]: shared macro for single-image handler
//! structs (PNG, JPEG, TIFF — anything backed by one
//! `image::DynamicImage` + a `ContentSource`).
//!
//! Generates the [`Handler`], [`Handle<Image>`],
//! [`IndexedHandle<Image>`] impls plus inherent constructors and a
//! `pub fn format() -> Format` descriptor that registers with
//! [`nvisy_codec::CodecRegistry`].
//!
//! [`Handler`]: nvisy_codec::handler::Handler
//! [`Handle<Image>`]: nvisy_codec::core::Handle
//! [`IndexedHandle<Image>`]: nvisy_codec::core::IndexedHandle

/// Implement [`Handler`], [`Handle<Image>`], [`IndexedHandle<Image>`],
/// and the shared inherent methods for an image handler struct that
/// holds a single `DynamicImage`, a `ContentSource`, and a streaming
/// cursor.
///
/// Each invocation also emits:
/// - a `pub const FORMAT_ID: FormatId` for the handler's stable id
/// - a `pub fn format() -> Format` descriptor wiring the matching
///   loader through [`LoaderAdapter`] into the registry
///
/// [`Handler`]: nvisy_codec::handler::Handler
/// [`Handle<Image>`]: nvisy_codec::core::Handle
/// [`IndexedHandle<Image>`]: nvisy_codec::core::IndexedHandle
/// [`LoaderAdapter`]: nvisy_codec::LoaderAdapter
#[macro_export]
macro_rules! impl_image_handler {
    (
        handler = $handler:ident,
        loader = $loader:ident,
        format_id = $format_id:literal,
        extensions = [$($ext:literal),+ $(,)?],
        content_types = [$($mime:literal),+ $(,)?],
        image_format = $img_fmt:expr,
        origin = $origin:literal,
        encode_span = $encode_name:literal $(,)?
    ) => {
        pub const FORMAT_ID: ::nvisy_codec::FormatId =
            ::nvisy_codec::FormatId::from_static($format_id);

        /// [`Format`] descriptor registered into
        /// [`nvisy_codec::CodecRegistry`].
        ///
        /// [`Format`]: ::nvisy_codec::Format
        pub fn format() -> ::nvisy_codec::Format {
            ::nvisy_codec::Format {
                id: FORMAT_ID.clone(),
                modality: ::nvisy_core::modality::ModalityKind::Image,
                extensions: vec![$($ext.into()),+],
                content_types: vec![$($mime.into()),+],
                loader: ::std::sync::Arc::new(
                    ::nvisy_codec::LoaderAdapter::new($loader::default()),
                ),
            }
        }

        impl ::nvisy_codec::handler::Handler for $handler {
            fn format(&self) -> ::nvisy_codec::FormatId {
                FORMAT_ID.clone()
            }

            fn source(&self) -> &::nvisy_core::content::ContentSource {
                &self.source
            }

            #[::tracing::instrument(name = $encode_name, skip_all, fields(output_bytes))]
            fn encode(
                &self,
            ) -> ::std::result::Result<::nvisy_core::content::ContentData, ::nvisy_core::Error>
            {
                let out = $crate::image::macros::encode_image(&self.image, $img_fmt, $origin)?;
                ::tracing::Span::current().record("output_bytes", out.len());
                let source = ::nvisy_core::content::ContentSource::new()
                    .with_parent(&self.source);
                Ok(::nvisy_core::content::ContentData::new(source, out.into()))
            }
        }

        #[::async_trait::async_trait]
        impl ::nvisy_codec::core::Handle<::nvisy_core::modality::Image> for $handler {
            async fn next_chunk(
                &mut self,
            ) -> ::std::result::Result<
                ::std::option::Option<
                    ::nvisy_codec::core::Chunk<::nvisy_core::modality::Image>,
                >,
                ::nvisy_core::Error,
            > {
                if self.yielded {
                    return Ok(None);
                }
                let (w, h) = (self.image.width(), self.image.height());
                let location = ::nvisy_core::modality::ImageLocation {
                    bounding_box: ::nvisy_core::primitive::BoundingBox {
                        x: 0.0,
                        y: 0.0,
                        width: w as f64,
                        height: h as f64,
                    },
                    polygon: None,
                    image_id: None,
                    page_number: None,
                };
                let bytes = $crate::image::macros::encode_image(
                    &self.image,
                    $img_fmt,
                    $origin,
                )?;
                let data = ::nvisy_core::modality::ImageData::new(
                    bytes,
                    ::nvisy_core::primitive::Dimensions::new(w, h),
                );
                self.yielded = true;
                Ok(Some(::nvisy_codec::core::Chunk {
                    location,
                    data,
                    embed: None,
                }))
            }
        }

        #[::async_trait::async_trait]
        impl ::nvisy_codec::core::IndexedHandle<::nvisy_core::modality::Image> for $handler {
            async fn read(
                &self,
                location: &::nvisy_core::modality::ImageLocation,
            ) -> ::std::result::Result<
                ::std::option::Option<::nvisy_core::modality::ImageData>,
                ::nvisy_core::Error,
            > {
                let bb = &location.bounding_box;
                let x = bb.x.max(0.0) as u32;
                let y = bb.y.max(0.0) as u32;
                let w = (bb.width as u32)
                    .min(self.image.width().saturating_sub(x));
                let h = (bb.height as u32)
                    .min(self.image.height().saturating_sub(y));
                if w == 0 || h == 0 {
                    return Ok(None);
                }
                let cropped = self.image.crop_imm(x, y, w, h);
                let bytes = $crate::image::macros::encode_image(
                    &cropped,
                    $img_fmt,
                    $origin,
                )?;
                Ok(Some(::nvisy_core::modality::ImageData::new(
                    bytes,
                    ::nvisy_core::primitive::Dimensions::new(w, h),
                )))
            }

            async fn redact(
                &mut self,
                redactions: ::nvisy_core::extraction::Redactions<
                    ::nvisy_core::modality::Image,
                >,
            ) -> ::std::result::Result<(), ::nvisy_core::Error> {
                for (location, replacement) in redactions.into_items() {
                    $crate::image::redact::apply(
                        &mut self.image,
                        &replacement,
                        location.bounding_box,
                    );
                }
                Ok(())
            }
        }

        impl $handler {
            /// Create a handler from an already-decoded image.
            pub fn new(image: ::image::DynamicImage) -> Self {
                Self {
                    source: ::nvisy_core::content::ContentSource::new(),
                    image,
                    yielded: false,
                }
            }

            /// Attach a content source for lineage tracking.
            pub fn with_source(
                mut self,
                source: ::nvisy_core::content::ContentSource,
            ) -> Self {
                self.source = source;
                self
            }

            /// Reference to the decoded image.
            pub fn image(&self) -> &::image::DynamicImage {
                &self.image
            }

            /// Rewind the streaming cursor so [`next_chunk`] yields
            /// the full-image chunk again.
            ///
            /// [`next_chunk`]: ::nvisy_codec::core::Handle::next_chunk
            pub fn rewind(&mut self) {
                self.yielded = false;
            }
        }
    };
}

/// Encode a [`DynamicImage`] into bytes via the given [`ImageFormat`].
pub(crate) fn encode_image(
    img: &::image::DynamicImage,
    fmt: ::image::ImageFormat,
    origin: &'static str,
) -> ::std::result::Result<::std::vec::Vec<u8>, ::nvisy_core::Error> {
    use ::std::io::Cursor;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, fmt)
        .map_err(|e| ::nvisy_core::Error::validation(format!("encode failed: {e}"), origin))?;
    Ok(buf.into_inner())
}
