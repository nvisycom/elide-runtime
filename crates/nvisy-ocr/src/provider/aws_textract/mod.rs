//! AWS Textract backend.
//!
//! Sends base64-encoded images to the `DetectDocumentText` action with
//! inline SigV4 request signing and parses word-level bounding boxes.

mod backend;
mod params;

pub use self::backend::AwsTextractBackend;
pub use self::params::AwsTextractParams;
