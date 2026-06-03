//! [`FilePrompt`]: load a [`Prompt`] from a TOML file.
//!
//! Mirrors Presidio's prompt-as-data model: the user-prompt template
//! plus the label map plus the labels-to-ignore set all live in a
//! single TOML file. Users swap behaviour by editing the file, not
//! by writing Rust. Templates use Jinja2 syntax via `minijinja`.
//!
//! # TOML schema
//!
//! ```toml
//! schema_version = 1
//!
//! [meta]
//! name = "ner-default"
//! modality = "text"   # or "image"
//!
//! # Optional. Maps model-emitted labels to canonical EntityKind.
//! # Use snake_case EntityKind names on the right-hand side.
//! [label_map]
//! person = "person_name"
//! email = "email_address"
//!
//! # Optional. Drop spans the model returned with any of these
//! # labels (case-sensitive).
//! labels_to_ignore = ["MISC", "O"]
//!
//! # Jinja2-syntax template. Available variables:
//! #   - text: source text (text modality only)
//! #   - image_b64: base64-encoded image bytes (image modality only)
//! #   - hints: list of { name, kind, value, snippet } (text) or
//! #            { name, kind, bbox: { x, y, width, height } } (image)
//! #   - labels: list of document context labels
//! template = """
//! Detect every sensitive entity in:
//! ---
//! {{ text }}
//! ---
//! {% if labels %}Labels: {{ labels | join(", ") }}{% endif %}
//! Return JSON: {"entities": [...]}.
//! """
//! ```

use std::fs;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use minijinja::{Environment, context};
use nvisy_core::entity::{Entity, EntityKind, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_core::modality::{Image, ImageLocation, Text, TextLocation};
use nvisy_core::primitive::Confidence;
use nvisy_core::{Error, LabelMap, RecognizerInput, Result};
use schemars::Schema;
use serde::Deserialize;

use super::candidates::{TextCandidates, VlmCandidates};
use super::localize::{UnresolvedCandidatePolicy, localize_all};
use super::prompt::Prompt;
use super::response_parser::parse_json;
use crate::backend::LlmResponse;

const DEFAULT_CONFIDENCE: f64 = 0.5;

/// File-driven [`Prompt`] impl.
///
/// Construct via [`from_toml_file`] or [`from_toml_str`]; the modality
/// (`M`) is checked against `meta.modality` at parse time.
///
/// [`from_toml_file`]: Self::from_toml_file
/// [`from_toml_str`]: Self::from_toml_str
pub struct FilePrompt<M> {
    template: String,
    label_map: LabelMap,
    labels_to_ignore: Vec<String>,
    env: Environment<'static>,
    _modality: PhantomData<fn() -> M>,
}

#[derive(Debug, Deserialize)]
struct PromptFile {
    #[allow(dead_code)]
    schema_version: Option<u32>,
    meta: PromptMeta,
    #[serde(default)]
    label_map: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    labels_to_ignore: Vec<String>,
    template: String,
}

#[derive(Debug, Deserialize)]
struct PromptMeta {
    #[allow(dead_code)]
    name: Option<String>,
    modality: String,
}

impl<M> FilePrompt<M> {
    fn from_parsed(parsed: PromptFile, expected_modality: &str) -> Result<Self> {
        if parsed.meta.modality != expected_modality {
            return Err(Error::validation(
                format!(
                    "prompt file modality is {:?}, expected {:?}",
                    parsed.meta.modality, expected_modality
                ),
                "file-prompt",
            ));
        }

        let mut label_map = LabelMap::new();
        if let Some(entries) = parsed.label_map {
            for (label, kind_str) in entries {
                let kind = kind_str.parse::<EntityKind>().map_err(|_| {
                    Error::validation(
                        format!("unknown EntityKind {kind_str:?} in label_map"),
                        "file-prompt",
                    )
                })?;
                label_map = label_map.with_entry(label, kind);
            }
        }

        let mut env = Environment::new();
        env.add_template_owned("prompt", parsed.template.clone())
            .map_err(|e| {
                Error::validation(format!("template compile error: {e}"), "file-prompt")
            })?;

        Ok(Self {
            template: parsed.template,
            label_map,
            labels_to_ignore: parsed.labels_to_ignore,
            env,
            _modality: PhantomData,
        })
    }

    /// Borrow the loaded template source.
    #[must_use]
    pub fn template(&self) -> &str {
        &self.template
    }

    /// Borrow the configured label map.
    #[must_use]
    pub fn label_map(&self) -> &LabelMap {
        &self.label_map
    }
}

impl FilePrompt<Text> {
    /// Load a text-modality prompt from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the file is missing, malformed,
    /// declares a non-`text` modality, references an unknown
    /// `EntityKind`, or contains an invalid Jinja2 template.
    pub fn from_toml_file(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path.as_ref())
            .map_err(|e| Error::validation(format!("reading prompt file: {e}"), "file-prompt"))?;
        Self::from_toml_str(&raw)
    }

    /// Load a text-modality prompt from a TOML string.
    ///
    /// # Errors
    ///
    /// See [`from_toml_file`].
    ///
    /// [`from_toml_file`]: Self::from_toml_file
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let parsed: PromptFile = toml::from_str(raw)
            .map_err(|e| Error::validation(format!("parsing prompt TOML: {e}"), "file-prompt"))?;
        Self::from_parsed(parsed, "text")
    }
}

impl FilePrompt<Image> {
    /// Load an image-modality prompt from a TOML file.
    ///
    /// # Errors
    ///
    /// Same as the text-modality loader.
    pub fn from_toml_file(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path.as_ref())
            .map_err(|e| Error::validation(format!("reading prompt file: {e}"), "file-prompt"))?;
        Self::from_toml_str(&raw)
    }

    /// Load an image-modality prompt from a TOML string.
    ///
    /// # Errors
    ///
    /// Same as the text-modality loader.
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let parsed: PromptFile = toml::from_str(raw)
            .map_err(|e| Error::validation(format!("parsing prompt TOML: {e}"), "file-prompt"))?;
        Self::from_parsed(parsed, "image")
    }
}

fn text_schema() -> &'static Schema {
    static CACHE: OnceLock<Schema> = OnceLock::new();
    CACHE.get_or_init(|| schemars::schema_for!(TextCandidates))
}

fn vlm_schema() -> &'static Schema {
    static CACHE: OnceLock<Schema> = OnceLock::new();
    CACHE.get_or_init(|| schemars::schema_for!(VlmCandidates))
}

impl Prompt<Text> for FilePrompt<Text> {
    fn build(&self, input: &RecognizerInput<Text>) -> String {
        let hints: Vec<_> = input
            .hints
            .iter()
            .map(|h| {
                let value = value_at(
                    input.data.text.as_str(),
                    h.location.start,
                    h.location.end,
                );
                let snippet = snippet_around(
                    input.data.text.as_str(),
                    h.location.start,
                    h.location.end,
                );
                context! {
                    name => h.name.as_deref().unwrap_or(""),
                    kind => h.entity_kind.map(|k| k.to_string()).unwrap_or_else(|| "unknown".to_owned()),
                    value => value,
                    snippet => snippet,
                }
            })
            .collect();
        let ctx = context! {
            text => input.data.text.as_str(),
            hints => hints,
            labels => input.labels.clone(),
        };
        self.env
            .get_template("prompt")
            .and_then(|t| t.render(ctx))
            .unwrap_or_default()
    }

    fn schema(&self) -> Option<&Schema> {
        Some(text_schema())
    }

    fn lift(&self, response: &LlmResponse, input: &RecognizerInput<Text>) -> Vec<Entity<Text>> {
        let Ok(parsed): Result<TextCandidates, _> = parse_json(&response.text) else {
            return Vec::new();
        };
        let text = input.data.text.as_str();
        let localized = localize_all(text, parsed.entities, UnresolvedCandidatePolicy::default());
        let model = ModelProvenance::new("llm".to_owned());

        let mut out = Vec::with_capacity(localized.len());
        for l in localized {
            let Some(entity_kind) = resolve_kind(
                l.candidate.entity_type,
                l.candidate.value.as_str(),
                &self.label_map,
                &self.labels_to_ignore,
            ) else {
                continue;
            };
            let raw = l.candidate.confidence.unwrap_or(DEFAULT_CONFIDENCE);
            let Some(confidence) = Confidence::new(raw.clamp(0.0, 1.0)) else {
                continue;
            };
            let location = TextLocation::new(l.start_offset, l.end_offset);
            let reason = format!("llm identified {entity_kind}");
            let step = TrailStep::recognition(
                "llm-ner",
                confidence,
                TrailProvenance::Model(model.clone()),
                reason,
            );
            let mut b = Entity::builder()
                .with_entity_kind(entity_kind)
                .with_trail(vec![step])
                .with_confidence(confidence)
                .with_location(location);
            if let Some(id) = l.candidate.entity_id {
                b = b.with_entity_id(id);
            }
            out.push(b.build().expect("required fields provided"));
        }
        out
    }
}

impl Prompt<Image> for FilePrompt<Image> {
    fn build(&self, input: &RecognizerInput<Image>) -> String {
        let image_b64 = STANDARD.encode(input.data.bytes.as_ref());
        let hints: Vec<_> = input
            .hints
            .iter()
            .map(|h| {
                let bbox = &h.location.bounding_box;
                context! {
                    name => h.name.as_deref().unwrap_or(""),
                    kind => h.entity_kind.map(|k| k.to_string()).unwrap_or_else(|| "unknown".to_owned()),
                    bbox => context! {
                        x => bbox.x,
                        y => bbox.y,
                        width => bbox.width,
                        height => bbox.height,
                    },
                }
            })
            .collect();
        let ctx = context! {
            image_b64 => image_b64,
            hints => hints,
            labels => input.labels.clone(),
        };
        self.env
            .get_template("prompt")
            .and_then(|t| t.render(ctx))
            .unwrap_or_default()
    }

    fn schema(&self) -> Option<&Schema> {
        Some(vlm_schema())
    }

    fn lift(&self, response: &LlmResponse, input: &RecognizerInput<Image>) -> Vec<Entity<Image>> {
        let Ok(parsed): Result<VlmCandidates, _> = parse_json(&response.text) else {
            return Vec::new();
        };
        let dims = input.data.dims;
        let model = ModelProvenance::new("llm".to_owned());

        let mut out = Vec::with_capacity(parsed.entities.len());
        for d in parsed.entities {
            let kind_str = d.entity_kind.to_string();
            if self.labels_to_ignore.iter().any(|l| l == &kind_str) {
                continue;
            }
            let entity_kind = self.label_map.lookup(&kind_str).unwrap_or(d.entity_kind);
            let raw = d.confidence.unwrap_or(DEFAULT_CONFIDENCE);
            let Some(confidence) = Confidence::new(raw.clamp(0.0, 1.0)) else {
                continue;
            };
            let bbox = d.bbox.to_pixel(dims);
            let location = ImageLocation::new(bbox);
            let reason = format!("vlm identified {entity_kind}");
            let step = TrailStep::recognition(
                "llm-vlm",
                confidence,
                TrailProvenance::Model(model.clone()),
                reason,
            );
            let entity = Entity::builder()
                .with_entity_kind(entity_kind)
                .with_trail(vec![step])
                .with_confidence(confidence)
                .with_location(location)
                .build()
                .expect("required fields provided");
            out.push(entity);
        }
        out
    }
}

fn resolve_kind(
    typed: Option<EntityKind>,
    value: &str,
    label_map: &LabelMap,
    labels_to_ignore: &[String],
) -> Option<EntityKind> {
    if let Some(kind) = typed {
        let s = kind.to_string();
        if labels_to_ignore.iter().any(|l| l == &s) {
            return None;
        }
        return Some(label_map.lookup(&s).unwrap_or(kind));
    }
    // No typed kind from the model — look up the literal value in
    // the label map (covers raw-string-label backends).
    if labels_to_ignore.iter().any(|l| l == value) {
        return None;
    }
    label_map.lookup(value)
}

fn value_at(text: &str, start: usize, end: usize) -> &str {
    if start < end
        && end <= text.len()
        && text.is_char_boundary(start)
        && text.is_char_boundary(end)
    {
        &text[start..end]
    } else {
        ""
    }
}

fn snippet_around(text: &str, start: usize, end: usize) -> &str {
    const HALF: usize = 80;
    let lo = floor_char_boundary(text, start.saturating_sub(HALF));
    let hi = ceil_char_boundary(text, (end + HALF).min(text.len()));
    &text[lo..hi]
}

fn floor_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn ceil_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}
