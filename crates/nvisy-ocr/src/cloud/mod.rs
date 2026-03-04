#[cfg(feature = "aws")]
mod aws_textract;
#[cfg(feature = "azure")]
mod azure_docai;
#[cfg(feature = "google")]
mod google_vision;

#[cfg(feature = "aws")]
pub use aws_textract::AwsTextractBackend;
#[cfg(feature = "azure")]
pub use azure_docai::AzureDocaiBackend;
#[cfg(feature = "google")]
pub use google_vision::GoogleVisionBackend;
