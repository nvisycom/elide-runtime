//! CV-side LLM agents: classification ([`CvAgent`]) and verification
//! ([`CvVerifyAgent`]).
//!
//! Both halves prompt a vision-capable LLM against an image.
//! [`CvAgent`] labels pre-computed CV detections (faces / plates /
//! signatures, etc.) with entity categories; [`CvVerifyAgent`]
//! validates upstream entity proposals against the image.

mod classify;
mod verify;

pub use self::classify::{CvAgent, CvDetection, CvEntity};
pub use self::verify::{CvVerifyAgent, VerificationCandidate};
