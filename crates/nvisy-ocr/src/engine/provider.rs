use nvisy_core::Result;
use nvisy_core::http::HttpClient;
use serde::{Deserialize, Serialize};

#[cfg(feature = "aws-textract")]
use crate::provider::{AwsTextractBackend, AwsTextractParams};
#[cfg(feature = "azure-docai")]
use crate::provider::{AzureDocaiBackend, AzureDocaiParams};
#[cfg(feature = "google-vision")]
use crate::provider::{GoogleVisionBackend, GoogleVisionParams};
use crate::provider::{PaddleXBackend, PaddleXParams, SuryaBackend, SuryaParams};

/// Union of all provider parameter types.
///
/// Each variant holds the configuration needed to construct one OCR backend.
/// Use [`into_engine`] to build a ready-to-use
/// `OcrEngine` from any variant.
///
/// [`into_engine`]: OcrProvider::into_engine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OcrProvider {
    /// Datalab Surya OCR.
    Surya(SuryaParams),
    /// PaddlePaddle PaddleX PP-OCRv5.
    PaddleX(PaddleXParams),
    /// AWS Textract.
    #[cfg(feature = "aws-textract")]
    #[cfg_attr(docsrs, doc(cfg(feature = "aws-textract")))]
    AwsTextract(AwsTextractParams),
    /// Azure Document Intelligence.
    #[cfg(feature = "azure-docai")]
    #[cfg_attr(docsrs, doc(cfg(feature = "azure-docai")))]
    AzureDocai(AzureDocaiParams),
    /// Google Cloud Vision.
    #[cfg(feature = "google-vision")]
    #[cfg_attr(docsrs, doc(cfg(feature = "google-vision")))]
    GoogleVision(GoogleVisionParams),
}

impl OcrProvider {
    /// Build an `OcrEngine` from these parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the default HTTP client cannot be built.
    pub fn into_engine(self) -> Result<super::OcrEngine> {
        Ok(match self {
            Self::Surya(p) => super::OcrEngine::new(SuryaBackend::new(p)?),
            Self::PaddleX(p) => super::OcrEngine::new(PaddleXBackend::new(p)?),
            #[cfg(feature = "aws-textract")]
            Self::AwsTextract(p) => super::OcrEngine::new(AwsTextractBackend::new(p)?),
            #[cfg(feature = "azure-docai")]
            Self::AzureDocai(p) => super::OcrEngine::new(AzureDocaiBackend::new(p)?),
            #[cfg(feature = "google-vision")]
            Self::GoogleVision(p) => super::OcrEngine::new(GoogleVisionBackend::new(p)?),
        })
    }

    /// Build an `OcrEngine` from these parameters using a pre-built HTTP client.
    ///
    /// This shares the caller's connection pool instead of creating a new one
    /// per backend.
    pub fn into_engine_with_client(self, client: HttpClient) -> super::OcrEngine {
        match self {
            Self::Surya(p) => super::OcrEngine::new(SuryaBackend::with_client(client, p)),
            Self::PaddleX(p) => super::OcrEngine::new(PaddleXBackend::with_client(client, p)),
            #[cfg(feature = "aws-textract")]
            Self::AwsTextract(p) => {
                super::OcrEngine::new(AwsTextractBackend::with_client(client, p))
            }
            #[cfg(feature = "azure-docai")]
            Self::AzureDocai(p) => super::OcrEngine::new(AzureDocaiBackend::with_client(client, p)),
            #[cfg(feature = "google-vision")]
            Self::GoogleVision(p) => {
                super::OcrEngine::new(GoogleVisionBackend::with_client(client, p))
            }
        }
    }
}
