//! PDF-to-image rendering via PDFium.
//!
//! PDFium is not thread-safe, so all rendering is serialised on a
//! dedicated single-thread [`rayon::ThreadPool`]. The [`PdfRenderer`]
//! binding is created once on first use via a `thread_local!` and
//! reused for all subsequent calls.

use std::cell::RefCell;
use std::sync::LazyLock;

use nvisy_core::Error;
use nvisy_ontology::primitive::Dpi;
use pdfium_render::prelude::*;

use crate::handler::image::ImageData;

/// Dedicated single-thread pool for PDFium operations.
static PDF_POOL: LazyLock<rayon::ThreadPool> = LazyLock::new(|| {
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .thread_name(|_| "pdfium".into())
        .build()
        .expect("failed to create PDFium thread pool")
});

thread_local! {
    static RENDERER: RefCell<Option<PdfRenderer>> = const { RefCell::new(None) };
}

/// Renders PDF pages to images for OCR processing.
///
/// Binding to the PDFium shared library is expensive, so the renderer
/// is lazily initialised on a dedicated thread and reused across calls.
/// Requires the PDFium shared library to be available at runtime
/// (bundled in the Docker image or installed on the host).
pub(super) struct PdfRenderer {
    pdfium: Pdfium,
}

impl PdfRenderer {
    /// Create a new renderer by binding to a system-provided PDFium library.
    fn new() -> Result<Self, Error> {
        let bindings = Pdfium::bind_to_system_library()
            .or_else(|_| Pdfium::bind_to_library("libpdfium"))
            .map_err(|e| {
                Error::runtime(
                    format!("failed to load PDFium library: {e}"),
                    "pdf_render",
                    false,
                )
            })?;
        Ok(Self {
            pdfium: Pdfium::new(bindings),
        })
    }

    /// Render all pages of a PDF to images at the given DPI.
    ///
    /// Dispatches work to a dedicated single-thread pool where the
    /// PDFium binding is lazily initialised and reused. A typical DPI
    /// value for OCR is [`Dpi::OCR`] (300).
    pub fn parallel_render(pdf_bytes: &[u8], dpi: Dpi) -> Result<Vec<ImageData>, Error> {
        let bytes = pdf_bytes.to_vec();

        PDF_POOL.install(|| {
            RENDERER.with_borrow_mut(|slot| {
                if slot.is_none() {
                    *slot = Some(PdfRenderer::new()?);
                }
                slot.as_ref().unwrap().render(&bytes, dpi)
            })
        })
    }

    /// Extract all embedded images from a PDF.
    ///
    /// Iterates over all page objects across every page, collecting
    /// image objects via [`PdfPageImageObject::get_raw_image`]. Each
    /// image is paired with its 0-based sequential index.
    pub fn extract_images(pdf_bytes: &[u8]) -> Result<Vec<ImageData>, Error> {
        let bytes = pdf_bytes.to_vec();

        PDF_POOL.install(|| {
            RENDERER.with_borrow_mut(|slot| {
                if slot.is_none() {
                    *slot = Some(PdfRenderer::new()?);
                }
                slot.as_ref().unwrap().extract(&bytes)
            })
        })
    }

    /// Extract embedded images using the bound PDFium instance.
    fn extract(&self, pdf_bytes: &[u8]) -> Result<Vec<ImageData>, Error> {
        let document = self
            .pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| Error::runtime(format!("failed to load PDF: {e}"), "pdf_render", false))?;

        let mut images = Vec::new();
        for page in document.pages().iter() {
            for object in page.objects().iter() {
                if let Some(image_object) = object.as_image_object() {
                    match image_object.get_raw_image() {
                        Ok(img) => images.push(ImageData::from(img)),
                        Err(e) => {
                            tracing::warn!(
                                target: "nvisy_codec::pdf_render",
                                error = %e,
                                "failed to extract embedded image, skipping",
                            );
                        }
                    }
                }
            }
        }

        Ok(images)
    }

    /// Render all pages using the bound PDFium instance.
    fn render(&self, pdf_bytes: &[u8], dpi: Dpi) -> Result<Vec<ImageData>, Error> {
        let document = self
            .pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| Error::runtime(format!("failed to load PDF: {e}"), "pdf_render", false))?;

        let config = PdfRenderConfig::new().scale_page_by_factor(dpi.scale_factor());

        let mut images = Vec::new();
        for page in document.pages().iter() {
            let bitmap = page.render_with_config(&config).map_err(|e| {
                Error::runtime(
                    format!("failed to render PDF page: {e}"),
                    "pdf_render",
                    false,
                )
            })?;
            images.push(ImageData::from(bitmap.as_image()));
        }

        Ok(images)
    }
}
