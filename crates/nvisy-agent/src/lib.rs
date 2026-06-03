#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod agent;
pub mod audio;
pub mod backend;
pub(crate) mod error;
mod recognition;

pub use self::recognition::{
    DefaultPrompt, FilePrompt, LlmRecognizer, LlmRecognizerBuilder, Prompt,
};
