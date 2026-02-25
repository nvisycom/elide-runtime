//! Internal rig `Tool` wrapper for [`CvProvider`].

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;
use serde_json::json;

use super::CvProvider;

/// Arguments for the CV tool call.
#[derive(Deserialize)]
pub(super) struct CvToolArgs {
    /// Base64-encoded image data.
    pub image_base64: String,
}

/// Error returned by the CV tool.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct CvToolError(String);

/// Rig `Tool` wrapper around a [`CvProvider`] implementation.
pub(super) struct CvRigTool<T: CvProvider>(Arc<T>);

impl<T: CvProvider> CvRigTool<T> {
    pub fn new(provider: T) -> Self {
        Self(Arc::new(provider))
    }
}

impl<T: CvProvider> Tool for CvRigTool<T> {
    const NAME: &'static str = "cv_detect_objects";

    type Error = CvToolError;
    type Args = CvToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Detect objects (faces, license plates, signatures) in an image \
                          using computer vision. Pass the image as a base64-encoded string."
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
            .map_err(|e| CvToolError(format!("invalid base64: {e}")))?;
        let detections = self
            .0
            .detect_objects(&bytes)
            .await
            .map_err(|e| CvToolError(e.to_string()))?;
        serde_json::to_string(&detections).map_err(|e| CvToolError(e.to_string()))
    }
}
