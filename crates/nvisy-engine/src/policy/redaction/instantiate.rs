//! [`Instantiate<M>`]: per-modality glue that turns a redaction
//! operator spec ([`DocumentModality::Redaction`]) into a runnable
//! [`Anonymizer<M>`] instance.
//!
//! Built-in arms construct a fresh operator from the rule's params
//! (no registry round-trip). The `Custom` arm looks up a
//! deployment-registered operator on the toolkit-side
//! [`RedactionRegistry<M>`].
//!
//! [`Anonymizer<M>`]: nvisy_toolkit::redaction::Anonymizer
//! [`DocumentModality::Redaction`]: crate::modality::DocumentModality::Redaction
//! [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry

use std::sync::Arc;

use nvisy_core::modality::{Audio, Image, Modality, Text};
use nvisy_core::{Error, Result};
use nvisy_toolkit::redaction::anonymizer::{Hash, Keep, Mask, Redact, Replace};
use nvisy_toolkit::redaction::{Anonymizer, RedactionRegistry};

use super::audio::AudioRedaction;
use super::image::ImageRedaction;
use super::text::TextRedaction;

const TARGET: &str = "nvisy_engine::policy::redaction";

/// Per-modality conversion from operator spec to runnable
/// [`Anonymizer<M>`]. Implemented by the per-modality redaction
/// enums in this module.
///
/// [`Anonymizer<M>`]: nvisy_toolkit::redaction::Anonymizer
pub trait Instantiate<M: Modality> {
    /// Resolve `self` to a runnable operator. Built-in arms
    /// construct a fresh instance; `Custom(id)` arms look up the
    /// operator in `registry`.
    ///
    /// # Errors
    ///
    /// Returns `Error::validation` when a `Custom` arm names an id
    /// that is not registered in `registry`.
    fn instantiate(&self, registry: &RedactionRegistry<M>) -> Result<Arc<dyn Anonymizer<M>>>;
}

impl Instantiate<Text> for TextRedaction {
    fn instantiate(&self, registry: &RedactionRegistry<Text>) -> Result<Arc<dyn Anonymizer<Text>>> {
        match self {
            Self::Replace { template } => Ok(Arc::new(Replace::new(template.clone()))),
            Self::Mask {
                mask_char,
                keep_prefix,
                keep_suffix,
            } => {
                let op = Mask::new(*mask_char)
                    .with_keep_prefix(*keep_prefix)
                    .with_keep_suffix(*keep_suffix);
                Ok(Arc::new(op))
            }
            Self::Hash { algorithm, salt } => {
                let mut op = Hash::new(*algorithm);
                if let Some(s) = salt {
                    op = op.with_salt(s.clone());
                }
                Ok(Arc::new(op))
            }
            Self::Redact => Ok(Arc::new(Redact)),
            Self::Keep => Ok(Arc::new(Keep)),
            Self::Custom { id } => registry.resolve_id(id).cloned().ok_or_else(|| {
                Error::validation(
                    format!("custom text anonymizer `{id}` is not registered"),
                    TARGET,
                )
            }),
        }
    }
}

impl Instantiate<Image> for ImageRedaction {
    fn instantiate(
        &self,
        registry: &RedactionRegistry<Image>,
    ) -> Result<Arc<dyn Anonymizer<Image>>> {
        match self {
            Self::Custom { id } => registry.resolve_id(id).cloned().ok_or_else(|| {
                Error::validation(
                    format!("custom image anonymizer `{id}` is not registered"),
                    TARGET,
                )
            }),
        }
    }
}

impl Instantiate<Audio> for AudioRedaction {
    fn instantiate(
        &self,
        registry: &RedactionRegistry<Audio>,
    ) -> Result<Arc<dyn Anonymizer<Audio>>> {
        match self {
            Self::Custom { id } => registry.resolve_id(id).cloned().ok_or_else(|| {
                Error::validation(
                    format!("custom audio anonymizer `{id}` is not registered"),
                    TARGET,
                )
            }),
        }
    }
}
