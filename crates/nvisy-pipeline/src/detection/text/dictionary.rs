//! Aho-Corasick dictionary-based entity detection layer.
//!
//! Supports both text spans ([`TxtSpan`]) and tabular spans
//! ([`CsvSpan`]).  Header spans in the CSV path are skipped
//! automatically.

use aho_corasick::AhoCorasick;
use serde::Deserialize;

use nvisy_codec::handler::{CsvSpan, Span, TxtSpan};
use nvisy_core::data::EntityCategory;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::path::ContentSource;
use nvisy_pattern::dictionaries;

use crate::ontology::{DetectionMethod, Entity, TabularLocation, TextLocation};

use crate::detection::context::ParallelContext;
use crate::detection::layer::{Detect, DetectionLayer};

/// Definition of a single dictionary for matching.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryDef {
    /// Dictionary name — e.g. `"nationalities"` for built-in, or a custom name.
    pub name: String,
    /// Entity category for matches from this dictionary.
    pub category: EntityCategory,
    /// Entity type label for matches (e.g. `"demographic"`, `"amount"`).
    pub entity_type: String,
    /// Custom values — empty when using a builtin dictionary.
    #[serde(default)]
    pub values: Vec<String>,
    /// Whether matching should be case-sensitive.
    #[serde(default)]
    pub case_sensitive: bool,
}

/// Typed parameters for [`DictionaryDetection`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryDetectionParams {
    /// One or more dictionaries to match against.
    pub dictionaries: Vec<DictionaryDef>,
    /// Confidence score assigned to dictionary matches.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.85
}

/// Scans text and tabular spans against Aho-Corasick automata built
/// from user-provided word lists and/or built-in gazetteers.
pub struct DictionaryDetection {
    confidence: f64,
    automata: Vec<(DictionaryDef, AhoCorasick, Vec<String>)>,
}

#[async_trait::async_trait]
impl DetectionLayer for DictionaryDetection {
    type Params = DictionaryDetectionParams;

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        if params.dictionaries.is_empty() {
            return Err(Error::new(
                ErrorKind::Validation,
                "at least one dictionary definition is required",
            ));
        }
        let automata = build_automata(&params.dictionaries)?;
        Ok(Self {
            confidence: params.confidence,
            automata,
        })
    }
}

#[async_trait::async_trait]
impl Detect<TxtSpan, String> for DictionaryDetection {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<TxtSpan, String>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            for (def, ac, values) in &self.automata {
                for mat in ac.find_iter(&span.data) {
                    let value = &values[mat.pattern().as_usize()];
                    let entity = Entity::new(
                        def.category.clone(),
                        &def.entity_type,
                        value.as_str(),
                        DetectionMethod::Dictionary,
                        self.confidence,
                    )
                    .with_text_location(TextLocation {
                        start_offset: mat.start(),
                        end_offset: mat.end(),
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    })
                    .with_parent(source);
                    entities.push(entity);
                }
            }
        }

        Ok(entities)
    }
}

#[async_trait::async_trait]
impl Detect<CsvSpan, String> for DictionaryDetection {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<CsvSpan, String>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            if span.id.header {
                continue;
            }
            if span.data.is_empty() {
                continue;
            }

            for (def, ac, values) in &self.automata {
                for mat in ac.find_iter(&span.data) {
                    let value = &values[mat.pattern().as_usize()];
                    let entity = Entity::new(
                        def.category.clone(),
                        &def.entity_type,
                        value.as_str(),
                        DetectionMethod::Dictionary,
                        self.confidence,
                    )
                    .with_tabular_location(TabularLocation {
                        row_index: span.id.row,
                        column_index: span.id.col,
                        start_offset: Some(mat.start()),
                        end_offset: Some(mat.end()),
                    })
                    .with_parent(source);
                    entities.push(entity);
                }
            }
        }

        Ok(entities)
    }
}

/// Resolve dictionary values (builtin or custom) and build Aho-Corasick automata.
fn build_automata(
    defs: &[DictionaryDef],
) -> Result<Vec<(DictionaryDef, AhoCorasick, Vec<String>)>, Error> {
    let mut result = Vec::with_capacity(defs.len());

    for def in defs {
        let values: Vec<String> = if !def.values.is_empty() {
            def.values.clone()
        } else {
            let builtin = dictionaries::get_builtin(&def.name).ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    format!("unknown builtin dictionary: {}", def.name),
                )
            })?;
            builtin.to_vec()
        };

        if values.is_empty() {
            continue;
        }

        let ac = aho_corasick::AhoCorasickBuilder::new()
            .ascii_case_insensitive(!def.case_sensitive)
            .build(&values)
            .map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to build automaton: {e}"))
            })?;

        result.push((def.clone(), ac, values));
    }

    Ok(result)
}
