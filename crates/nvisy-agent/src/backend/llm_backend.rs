//! [`LlmBackend`]: the modality-agnostic LLM backend trait.
//!
//! The backend is the swappable LLM plumbing — provider dispatch,
//! structured-output handling, retries, usage tracking. It takes a
//! prompt + an optional output schema and returns the model's reply
//! text. It does *not* know about modalities; modality-specific work
//! (prompt construction, response → `Entity<M>` lifting) lives in
//! the recognizer.
//!
//! Object-safe: recognizers hold `Arc<dyn LlmBackend>` and dispatch
//! per call.

use async_trait::async_trait;
use nvisy_core::Result;

/// One per-call LLM request handed to a [`LlmBackend`].
#[derive(Debug, Clone)]
pub struct LlmRequest<'a> {
    /// Fully-rendered user prompt. The recognizer is responsible for
    /// folding the source text, hints, labels, and any base64-encoded
    /// binary payloads (images, audio) into this string.
    pub prompt: &'a str,
    /// Optional JSON schema the backend asks the model to constrain
    /// output against. Backends that support structured output
    /// (rig's `output_schema`) use it; backends that don't, ignore
    /// it. `None` means the recognizer's prompt is responsible for
    /// describing the expected output shape inline.
    pub schema: Option<&'a schemars::Schema>,
}

/// One per-call LLM response from a [`LlmBackend`].
///
/// Wraps the model's reply text verbatim. The recognizer
/// deserialises it (and applies whatever markdown-fence /
/// sentinel-text forgiveness its [`Prompt<M>`] needs) on the way
/// out.
///
/// [`Prompt<M>`]: crate::recognition::Prompt
#[derive(Debug, Clone, Default)]
pub struct LlmResponse {
    /// Model's reply text, verbatim.
    pub text: String,
}

impl LlmResponse {
    /// Construct a response from raw text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Per-call LLM backend.
///
/// Implemented by everything that turns a `(prompt, schema)` pair
/// into the model's text reply — rig-backed providers (OpenAI,
/// Anthropic, Gemini, Ollama), externalised inference gateways, the
/// in-process no-op test stub.
#[async_trait]
pub trait LlmBackend: Send + Sync + 'static {
    /// Send `request` to the model and return its reply.
    ///
    /// # Errors
    ///
    /// Returns the underlying transport / provider / parse error.
    async fn predict(&self, request: LlmRequest<'_>) -> Result<LlmResponse>;

    /// Model name the backend is configured to call. Recognizers
    /// stamp this into entity trail provenance so post-hoc analysis
    /// can attribute scores to a specific model.
    fn model(&self) -> &str;
}
