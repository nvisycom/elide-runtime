//! Internal rig `Tool` wrapper for [`OcrProvider`].

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;
use serde_json::json;

use super::OcrProvider;

/// Arguments for the OCR tool call.
#[derive(Deserialize)]
pub(super) struct OcrToolArgs {
    /// Base64-encoded image data.
    pub image_base64: String,
}

/// Error returned by the OCR tool.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct OcrToolError(String);

/// Rig `Tool` wrapper around an [`OcrProvider`] implementation.
pub(super) struct OcrRigTool<T: OcrProvider>(pub Arc<T>);

impl<T: OcrProvider> Tool for OcrRigTool<T> {
    const NAME: &'static str = "ocr_extract_text";

    type Error = OcrToolError;
    type Args = OcrToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Extract text from an image using OCR. \
                          Pass the image as a base64-encoded string."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "image_base64": {
                        "type": "string",
                        "description": "Base64-encoded image data"
                    }
                },
                "required": ["image_base64"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let bytes = STANDARD
            .decode(&args.image_base64)
            .map_err(|e| OcrToolError(format!("invalid base64: {e}")))?;
        self.0
            .extract_text(&bytes)
            .await
            .map_err(|e| OcrToolError(e.to_string()))
    }
}
