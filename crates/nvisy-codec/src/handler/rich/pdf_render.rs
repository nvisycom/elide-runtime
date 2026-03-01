//! PDF-to-image rendering via PDFium.

use image::DynamicImage;
use pdfium_render::prelude::*;

use nvisy_core::Error;

/// Renders PDF pages to images for OCR processing.
///
/// Requires the PDFium shared library to be available at runtime
/// (bundled in the Docker image or installed on the host).
pub struct PdfRenderer {
    pdfium: Pdfium,
}

impl PdfRenderer {
    /// Create a new renderer by binding to a system-provided PDFium library.
    pub fn new() -> Result<Self, Error> {
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
    /// Each page is rendered as a separate [`DynamicImage`].  A typical
    /// DPI value for OCR is 300.
    pub fn render_pages(&self, pdf_bytes: &[u8], dpi: u16) -> Result<Vec<DynamicImage>, Error> {
        let document = self
            .pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| {
                Error::runtime(
                    format!("failed to load PDF: {e}"),
                    "pdf_render",
                    false,
                )
            })?;

        // PDF points are 1/72 inch; scale factor = target_dpi / 72.
        let scale = f32::from(dpi) / 72.0;

        let config = PdfRenderConfig::new().scale_page_by_factor(scale);

        let mut images = Vec::new();
        for page in document.pages().iter() {
            let bitmap = page.render_with_config(&config).map_err(|e| {
                Error::runtime(
                    format!("failed to render PDF page: {e}"),
                    "pdf_render",
                    false,
                )
            })?;
            images.push(bitmap.as_image());
        }

        Ok(images)
    }
}
