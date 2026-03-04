//! All OCR backend implementations and their parameter types.

mod datalab_surya;
mod mindee_doctr;
mod paddle_paddlex;

pub use datalab_surya::{SuryaBackend, SuryaParams};
pub use mindee_doctr::{DoctrBackend, DoctrParams};
pub use paddle_paddlex::{PaddleXBackend, PaddleXParams};

#[cfg(feature = "aws")]
#[cfg_attr(docsrs, doc(cfg(feature = "aws")))]
mod aws_textract;
#[cfg(feature = "azure")]
#[cfg_attr(docsrs, doc(cfg(feature = "azure")))]
mod azure_docai;
#[cfg(feature = "google")]
#[cfg_attr(docsrs, doc(cfg(feature = "google")))]
mod google_vision;

#[cfg(feature = "aws")]
#[cfg_attr(docsrs, doc(cfg(feature = "aws")))]
pub use aws_textract::{AwsTextractBackend, AwsTextractParams};
#[cfg(feature = "azure")]
#[cfg_attr(docsrs, doc(cfg(feature = "azure")))]
pub use azure_docai::{AzureDocaiBackend, AzureDocaiParams};
#[cfg(feature = "google")]
#[cfg_attr(docsrs, doc(cfg(feature = "google")))]
pub use google_vision::{GoogleVisionBackend, GoogleVisionParams};
