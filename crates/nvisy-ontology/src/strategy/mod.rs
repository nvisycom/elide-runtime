//! Redaction strategies for text, image, and audio modalities.
//!
//! This module defines *how* sensitive data is redacted once detected. It
//! provides two complementary layers that separate intent from configuration:
//!
//! ## Methods
//!
//! [`RedactionMethod`] (and its per-modality variants [`TextRedactionMethod`],
//! [`ImageRedactionMethod`], [`AudioRedactionMethod`]) are lightweight enum
//! identifiers that name a redaction technique — mask, blur, silence, etc. —
//! without any configuration. LLM agents return these when recommending a
//! strategy for a detected entity.
//!
//! ## Strategies
//!
//! [`RedactionStrategy`] (and its per-modality variants
//! [`TextRedactionStrategy`], [`ImageRedactionStrategy`],
//! [`AudioRedactionStrategy`]) pair a method with its parameters: mask
//! character, blur sigma, encryption key, pixel block size, and so on.
//! Policy rules and the redaction engine operate on these.
//!
//! The split lets agents reason about *what* to do without needing to know
//! default values, while the engine always receives fully-specified
//! configuration.

mod input;
mod method;

pub use input::{
    AudioRedactionStrategy, DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR,
    DEFAULT_PIXELATE_BLOCK_SIZE, ImageRedactionStrategy, RedactionStrategy,
    TextRedactionStrategy,
};
pub use method::{
    AudioRedactionMethod, ImageRedactionMethod, RedactionMethod, TextRedactionMethod,
};
