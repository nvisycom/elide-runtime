//! Post-deserialization validation and environment variable resolution.

use nvisy_core::Error;
use validator::Validate;

use super::RuntimeConfig;

impl RuntimeConfig {
    /// Validate all configuration sections.
    ///
    /// Checks structural constraints (e.g. `channel_buffer >= 1`,
    /// retry/timeout ranges) using the `validator` crate. Should be
    /// called once after deserialization and after any merge.
    ///
    /// # Errors
    ///
    /// Returns a validation error listing all constraint violations.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(ref engine) = self.engine {
            engine
                .validate()
                .map_err(|e| Error::validation(format!("engine: {e}"), "config"))?;
        }
        Ok(())
    }

    /// Resolve `api_key` fields from environment variables.
    ///
    /// For each subsystem provider that holds an `api_key`, if the
    /// current value is empty, the corresponding environment variable
    /// is checked:
    ///
    /// - `[llm.provider]`  → `NVISY_LLM_API_KEY`
    /// - `[stt.provider]`  → `NVISY_STT_API_KEY`
    /// - `[tts.provider]`  → `NVISY_TTS_API_KEY`
    /// - `[ocr.provider]`  → `NVISY_OCR_API_KEY`
    ///
    /// Call this once after loading from TOML, before passing the
    /// config to the engine.
    pub fn resolve_env(&mut self) {
        if let Some(ref mut llm) = self.llm {
            resolve_agent_provider_key(&mut llm.provider, "NVISY_LLM_API_KEY");
        }
        if let Some(ref mut stt) = self.stt {
            resolve_stt_provider_key(&mut stt.provider, "NVISY_STT_API_KEY");
        }
        if let Some(ref mut tts) = self.tts {
            resolve_tts_provider_key(&mut tts.provider, "NVISY_TTS_API_KEY");
        }
        if let Some(ref mut ocr) = self.ocr {
            resolve_ocr_provider_key(&mut ocr.provider, "NVISY_OCR_API_KEY");
        }
    }
}

#[allow(unused_variables, unused_imports)]
fn resolve_agent_provider_key(
    provider: &mut Option<nvisy_rig::agent::AgentProvider>,
    env_var: &str,
) {
    use nvisy_rig::agent::AgentProvider;

    let Some(p) = provider else { return };
    match p {
        #[cfg(feature = "openai")]
        AgentProvider::OpenAi(auth) => fill_key_from_env(&mut auth.api_key, env_var),
        #[cfg(feature = "anthropic")]
        AgentProvider::Anthropic(auth) => fill_key_from_env(&mut auth.api_key, env_var),
        #[cfg(feature = "google")]
        AgentProvider::Gemini(auth) => fill_key_from_env(&mut auth.api_key, env_var),
        _ => {}
    }
}

#[allow(unused_variables, unused_imports)]
fn resolve_stt_provider_key(provider: &mut Option<nvisy_rig::audio::SttProvider>, env_var: &str) {
    use nvisy_rig::audio::SttProvider;

    let Some(p) = provider else { return };
    match p {
        #[cfg(feature = "openai")]
        SttProvider::OpenAi(auth) => fill_key_from_env(&mut auth.api_key, env_var),
        _ => {}
    }
}

#[allow(unused_variables, unused_imports)]
fn resolve_tts_provider_key(provider: &mut Option<nvisy_rig::audio::TtsProvider>, env_var: &str) {
    use nvisy_rig::audio::TtsProvider;

    let Some(p) = provider else { return };
    match p {
        #[cfg(feature = "openai")]
        TtsProvider::OpenAi(auth) => fill_key_from_env(&mut auth.api_key, env_var),
        _ => {}
    }
}

#[allow(unused_variables, unused_imports)]
fn resolve_ocr_provider_key(provider: &mut Option<nvisy_ocr::OcrProvider>, env_var: &str) {
    use nvisy_ocr::OcrProvider;

    let Some(p) = provider else { return };
    match p {
        #[cfg(feature = "google")]
        OcrProvider::GoogleVision(params) => fill_key_from_env(&mut params.api_key, env_var),
        #[cfg(feature = "microsoft")]
        OcrProvider::AzureDocai(params) => fill_key_from_env(&mut params.api_key, env_var),
        _ => {}
    }
}

/// If `key` is empty, try to populate it from the environment variable.
#[allow(dead_code)]
pub(super) fn fill_key_from_env(key: &mut String, env_var: &str) {
    if key.is_empty() {
        if let Ok(val) = std::env::var(env_var) {
            if !val.is_empty() {
                *key = val;
            }
        }
    }
}
