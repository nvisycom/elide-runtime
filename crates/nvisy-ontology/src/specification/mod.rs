//! Redaction specifications for all modalities.
//!
//! This module contains two layers:
//!
//! - **Methods** ([`TextRedactionMethod`], [`ImageRedactionMethod`],
//!   [`AudioRedactionMethod`], [`RedactionMethod`]) — flat enums naming
//!   *what kind* of redaction to apply, without configuration. These are
//!   returned by LLM agents when recommending a strategy.
//!
//! - **Inputs** ([`TextRedactionInput`], [`ImageRedactionInput`],
//!   [`AudioRedactionInput`], [`RedactionInput`]) — tagged enums carrying
//!   method-specific configuration (mask char, blur sigma, etc.). These
//!   are submitted to the redaction engine for execution.
//!
//! The [`RedactorInput`] struct carries entity context passed *into* a
//! redactor agent so it can choose the right method.

mod input;
mod method;

pub use input::{
    AudioRedactionInput, ImageRedactionInput, RedactionInput, RedactorInput, TextRedactionInput,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
pub use method::{
    AudioRedactionMethod, ImageRedactionMethod, RedactionMethod, TextRedactionMethod,
};
