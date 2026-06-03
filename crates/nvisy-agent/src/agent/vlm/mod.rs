//! Vision-language model agent ([`VlmAgent`]) — direct image entity
//! discovery. Takes an image plus pixel [`Dimensions`] and asks the
//! VLM to draw bounding boxes around sensitive content.
//!
//! [`Dimensions`]: nvisy_core::primitive::Dimensions

mod detect;

pub use self::detect::VlmAgent;
