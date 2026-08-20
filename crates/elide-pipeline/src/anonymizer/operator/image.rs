//! Image-modality operator builder.
//!
//! Symmetric with [`super::text`]: builds an [`ImageOp`] from an
//! `ImageRedaction` spec, dispatches it onto a [`Target`] with
//! the right concrete elide operator type. Image operators do
//! not cross modalities, so the vocabulary lives on its own.

use elide::modality::image::Image;
use elide::redaction::Anonymizer;
use elide::redaction::operators::{Blackbox, Blur, Erase, Keep, Pixelate};
use elide_governance::redaction::ImageRedaction;

use crate::anonymizer::compile::Target;

/// Discriminated builder result so [`Target::attach_with`] can
/// attach the right concrete operator type. Same reason as
/// [`super::text::TextOp`]: [`Anonymizer::with_label`] takes
/// `O: Operator<M> + 'static` by value, so a `Box<dyn Operator>`
/// won't do.
///
/// [`Anonymizer::with_label`]: elide::redaction::Anonymizer::with_label
pub(in crate::anonymizer) enum ImageOp {
    Erase,
    Keep,
    Blur(Blur),
    Pixelate(Pixelate),
    Blackbox(Blackbox),
}

impl ImageOp {
    /// Attach `self` to `target`.
    pub(in crate::anonymizer) fn attach_to(self, target: Target<'_, Image>) -> Anonymizer<Image> {
        match self {
            ImageOp::Erase => target.attach_with(Erase),
            ImageOp::Keep => target.attach_with(Keep),
            ImageOp::Blur(op) => target.attach_with(op),
            ImageOp::Pixelate(op) => target.attach_with(op),
            ImageOp::Blackbox(op) => target.attach_with(op),
        }
    }
}

impl From<&ImageRedaction> for ImageOp {
    fn from(spec: &ImageRedaction) -> Self {
        match spec {
            ImageRedaction::Erase => Self::Erase,
            ImageRedaction::Keep => Self::Keep,
            ImageRedaction::Blur { sigma } => Self::Blur(Blur::new(*sigma)),
            ImageRedaction::Pixelate { block_size } => Self::Pixelate(Pixelate::new(*block_size)),
            ImageRedaction::Blackbox { color } => Self::Blackbox(Blackbox::new(*color)),
        }
    }
}
