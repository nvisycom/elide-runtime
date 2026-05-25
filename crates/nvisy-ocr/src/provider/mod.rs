//! All OCR backend implementations and their parameter types.
//!
//! Two HTTP sidecar backends (`PaddleXBackend`, `SuryaBackend`) were
//! removed pending the externalized inference layer landing (see
//! `nvisycom/runtime#194`). An HTTP backend pointing at
//! `nvisycom/inference` lands in a follow-up PR.

#[cfg(feature = "aws-textract")]
#[cfg_attr(docsrs, doc(cfg(feature = "aws-textract")))]
mod aws_textract;
#[cfg(feature = "azure-docai")]
#[cfg_attr(docsrs, doc(cfg(feature = "azure-docai")))]
mod azure_docai;
#[cfg(feature = "google-vision")]
#[cfg_attr(docsrs, doc(cfg(feature = "google-vision")))]
mod google_vision;

#[cfg(feature = "aws-textract")]
#[cfg_attr(docsrs, doc(cfg(feature = "aws-textract")))]
pub use self::aws_textract::{AwsTextractBackend, AwsTextractParams};
#[cfg(feature = "azure-docai")]
#[cfg_attr(docsrs, doc(cfg(feature = "azure-docai")))]
pub use self::azure_docai::{AzureDocaiBackend, AzureDocaiParams};
#[cfg(feature = "google-vision")]
#[cfg_attr(docsrs, doc(cfg(feature = "google-vision")))]
pub use self::google_vision::{GoogleVisionBackend, GoogleVisionParams};
