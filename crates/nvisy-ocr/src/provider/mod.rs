//! All OCR backend implementations and their parameter types.

mod datalab_surya;
mod paddle_paddlex;

pub use datalab_surya::{SuryaBackend, SuryaParams};
pub use paddle_paddlex::{PaddleXBackend, PaddleXParams};

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
pub use aws_textract::{AwsTextractBackend, AwsTextractParams};
#[cfg(feature = "azure-docai")]
#[cfg_attr(docsrs, doc(cfg(feature = "azure-docai")))]
pub use azure_docai::{AzureDocaiBackend, AzureDocaiParams};
#[cfg(feature = "google-vision")]
#[cfg_attr(docsrs, doc(cfg(feature = "google-vision")))]
pub use google_vision::{GoogleVisionBackend, GoogleVisionParams};
