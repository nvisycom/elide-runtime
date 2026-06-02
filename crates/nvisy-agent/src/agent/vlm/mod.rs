//! Vision-language model agents:
//!
//! - [`VlmAgent`] — direct image entity discovery. Takes an image
//!   plus pixel [`Dimensions`] and asks the VLM to draw bounding
//!   boxes around sensitive content.
//! - [`VlmVerifyAgent`] — validates already-detected image
//!   entities against the source image.
//!
//! [`Dimensions`]: nvisy_core::primitive::Dimensions

mod detect;
mod verify;

pub use self::detect::VlmAgent;
pub use self::verify::{VerificationCandidate, VlmVerifyAgent};
