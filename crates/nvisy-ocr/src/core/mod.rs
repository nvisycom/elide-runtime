//! Core OCR contract: the [`Backend`] trait, the shared input /
//! output types, and the per-call [`OcrParams`] hints.
//!
//! Backend implementations live in [`crate::backend`]; that module
//! also hosts the [`OcrBackend`] config enum that dispatches to a
//! concrete backend.
//!
//! [`OcrBackend`]: crate::backend::OcrBackend

mod input;
mod output;
mod params;

use nvisy_core::Error;
pub use nvisy_core::media::ImageFormat;

pub use self::input::ImageInput;
pub use self::output::ImageOutput;
pub use self::params::OcrParams;

/// The OCR backend contract.
///
/// Implementations send an image to an OCR service and return
/// hierarchical [`ImageOutput`] results with page/block/line/word
/// structure.
///
/// Confidence values **must** be normalised to `0.0..=1.0` before
/// being placed on words. Backends whose upstream API uses a
/// different scale are responsible for converting.
///
/// Implementors **must** provide [`run`]. The default
/// [`run_batch`] impl dispatches the inputs concurrently via
/// `futures::join_all`; backends with native batching (such as a
/// single network round-trip) should override it.
///
/// [`run`]: Self::run
/// [`run_batch`]: Self::run_batch
#[async_trait::async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Run OCR on a single image under `params`.
    async fn run(&self, image: &ImageInput, params: OcrParams<'_>)
        -> Result<ImageOutput, Error>;

    /// Run OCR on each of `images` under one shared [`OcrParams`].
    ///
    /// The default impl dispatches concurrently via
    /// `futures::join_all` — for one-at-a-time HTTP backends this
    /// is the right baseline. Backends with a native batch API
    /// override it to issue a single round-trip.
    async fn run_batch(
        &self,
        images: &[ImageInput],
        params: OcrParams<'_>,
    ) -> Result<Vec<ImageOutput>, Error> {
        let futures: Vec<_> = images.iter().map(|img| self.run(img, params)).collect();
        let results: Vec<Result<ImageOutput, Error>> = futures::future::join_all(futures).await;
        results.into_iter().collect()
    }
}
