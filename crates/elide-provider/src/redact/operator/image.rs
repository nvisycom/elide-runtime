//! Image-modality operator builder.
//!
//! Symmetric with [`super::text`]: builds an [`ImageOp`] from an
//! `ImageRedaction` spec, dispatches it onto a [`Target`] with
//! the right concrete elide operator type. Image operators do
//! not cross modalities, so the vocabulary lives on its own.

use elide::modality::image::Image;
use elide::redaction::Anonymizer;
use elide::redaction::operators::{Blur, Cover, Keep, Pixelate};
use elide::{Error, Result};
use elide_schema::redaction::ImageRedaction;

use crate::redact::target::Target;

/// Discriminated builder result so [`Target::attach_with`] can
/// attach the right concrete operator type. Same reason as
/// [`super::text::TextOp`]: [`Anonymizer::with_label`] takes
/// `O: Operator<M> + 'static` by value, so a `Box<dyn Operator>`
/// won't do.
///
/// [`Anonymizer::with_label`]: elide::redaction::Anonymizer::with_label
pub(in crate::redact) enum ImageOp {
    Keep,
    Blur(Blur),
    Pixelate(Pixelate),
    Cover(Cover),
}

impl ImageOp {
    /// Attach `self` to `target`.
    pub(in crate::redact) fn attach_to(self, target: Target<'_, Image>) -> Anonymizer<Image> {
        match self {
            ImageOp::Keep => target.attach_with(Keep),
            ImageOp::Blur(op) => target.attach_with(op),
            ImageOp::Pixelate(op) => target.attach_with(op),
            ImageOp::Cover(op) => target.attach_with(op),
        }
    }
}

impl TryFrom<&ImageRedaction> for ImageOp {
    type Error = Error;

    /// Build the operator `spec` names.
    ///
    /// Fails with [`Configuration`](elide::ErrorKind::Configuration)
    /// when elide rejects a parameter: a blur under one pixel or a
    /// mosaic block under two would leave the region readable.
    fn try_from(spec: &ImageRedaction) -> Result<Self> {
        Ok(match spec {
            ImageRedaction::Keep => Self::Keep,
            ImageRedaction::Blur { sigma } => Self::Blur(Blur::new(*sigma)?),
            ImageRedaction::Pixelate { block_size } => Self::Pixelate(Pixelate::new(*block_size)?),
            ImageRedaction::Cover { color } => Self::Cover(Cover::new(*color)),
        })
    }
}
