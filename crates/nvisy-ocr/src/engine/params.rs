use serde::{Deserialize, Serialize};

/// Union of all provider parameter types.
///
/// Each variant holds the configuration needed to construct one OCR backend.
/// Use [`into_engine`](EngineParams::into_engine) to build a ready-to-use
/// [`Engine`] from any variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineParams {
    /// Datalab Surya OCR.
    Surya(crate::provider::SuryaParams),
    /// Mindee DocTR.
    Doctr(crate::provider::DoctrParams),
    /// PaddlePaddle PaddleX PP-OCRv5.
    PaddleX(crate::provider::PaddleXParams),
    /// AWS Textract.
    #[cfg(feature = "aws-textract")]
    #[cfg_attr(docsrs, doc(cfg(feature = "aws-textract")))]
    AwsTextract(crate::provider::AwsTextractParams),
    /// Azure Document Intelligence.
    #[cfg(feature = "azure-docai")]
    #[cfg_attr(docsrs, doc(cfg(feature = "azure-docai")))]
    AzureDocai(crate::provider::AzureDocaiParams),
    /// Google Cloud Vision.
    #[cfg(feature = "google-vision")]
    #[cfg_attr(docsrs, doc(cfg(feature = "google-vision")))]
    GoogleVision(crate::provider::GoogleVisionParams),
}

impl EngineParams {
    /// Build an [`Engine`] from these parameters.
    pub fn into_engine(self) -> super::Engine {
        use crate::provider::*;

        match self {
            Self::Surya(p) => super::Engine::new(SuryaBackend::new(p)),
            Self::Doctr(p) => super::Engine::new(DoctrBackend::new(p)),
            Self::PaddleX(p) => super::Engine::new(PaddleXBackend::new(p)),
            #[cfg(feature = "aws-textract")]
            Self::AwsTextract(p) => super::Engine::new(AwsTextractBackend::new(p)),
            #[cfg(feature = "azure-docai")]
            Self::AzureDocai(p) => super::Engine::new(AzureDocaiBackend::new(p)),
            #[cfg(feature = "google-vision")]
            Self::GoogleVision(p) => super::Engine::new(GoogleVisionBackend::new(p)),
        }
    }
}
