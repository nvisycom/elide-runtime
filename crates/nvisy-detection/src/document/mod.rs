//! Document-level detection actions.

pub mod checksum;
pub mod manual;

pub use checksum::{DetectChecksumAction, DetectChecksumParams};
pub use manual::{DetectManualAction, DetectManualParams};
