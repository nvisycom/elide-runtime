//! Aho-Corasick dictionary-based entity detection action.

use aho_corasick::AhoCorasick;
use serde::Deserialize;

use nvisy_codec::handler::{TxtHandler, CsvHandler};
use nvisy_codec::document::Document;
use nvisy_core::data::EntityCategory;
use crate::ontology::{DetectionMethod, Entity, TabularLocation, TextLocation};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_pattern::dictionaries;

use crate::action::Action;

/// Definition of a single dictionary for matching.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryDef {
    /// Dictionary name — `"builtin:first_names"` for built-in, or a custom name.
    pub name: String,
    /// Entity category for matches from this dictionary.
    pub category: EntityCategory,
    /// Entity type label for matches (e.g. `"first_name"`, `"medical_term"`).
    pub entity_type: String,
    /// Custom values — empty when using a builtin dictionary.
    #[serde(default)]
    pub values: Vec<String>,
    /// Whether matching should be case-sensitive.
    #[serde(default)]
    pub case_sensitive: bool,
}

/// Typed parameters for [`DetectDictionaryAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectDictionaryParams {
    /// One or more dictionaries to match against.
    pub dictionaries: Vec<DictionaryDef>,
    /// Confidence score assigned to dictionary matches.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.85
}

/// Scans document text and tabular cells against Aho-Corasick automata
/// built from user-provided word lists and/or built-in gazetteers.
pub struct DetectDictionaryAction {
    params: DetectDictionaryParams,
    automata: Vec<(DictionaryDef, AhoCorasick, Vec<String>)>,
}

#[async_trait::async_trait]
impl Action for DetectDictionaryAction {
    type Params = DetectDictionaryParams;
    type Input = (Vec<Document<TxtHandler>>, Vec<Document<CsvHandler>>);
    type Output = Vec<Entity>;

    const ID: &str = "detect-dictionary";

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        if params.dictionaries.is_empty() {
            return Err(Error::new(
                ErrorKind::Validation,
                "at least one dictionary definition is required",
            ));
        }
        let automata = build_automata(&params.dictionaries)?;
        Ok(Self { params, automata })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Vec<Entity>, Error> {
        let (text_docs, tabular_docs) = input;
        let confidence = self.params.confidence;
        let mut entities = Vec::new();

        // Text content matching
        for doc in &text_docs {
            let lines = doc.handler().lines();
            let mut content = lines.join("\n");
            if doc.handler().trailing_newline() {
                content.push('\n');
            }

            for (def, ac, values) in &self.automata {
                for mat in ac.find_iter(&content) {
                    let value = &values[mat.pattern().as_usize()];
                    let entity = Entity::new(
                        def.category.clone(),
                        &def.entity_type,
                        value.as_str(),
                        DetectionMethod::Dictionary,
                        confidence,
                    )
                    .with_text_location(TextLocation {
                        start_offset: mat.start(),
                        end_offset: mat.end(),
                        context_start_offset: None,
                        context_end_offset: None,
                        element_id: None,
                        page_number: None,
                    })
                    .with_parent(&doc.source);
                    entities.push(entity);
                }
            }
        }

        // Tabular content matching
        for doc in &tabular_docs {
            for (row_idx, row) in doc.handler().rows().iter().enumerate() {
                for (col_idx, cell) in row.iter().enumerate() {
                    if cell.is_empty() {
                        continue;
                    }
                    for (def, ac, values) in &self.automata {
                        for mat in ac.find_iter(cell) {
                            let value = &values[mat.pattern().as_usize()];
                            let entity = Entity::new(
                                def.category.clone(),
                                &def.entity_type,
                                value.as_str(),
                                DetectionMethod::Dictionary,
                                confidence,
                            )
                            .with_tabular_location(TabularLocation {
                                row_index: row_idx,
                                column_index: col_idx,
                                start_offset: Some(mat.start()),
                                end_offset: Some(mat.end()),
                            })
                            .with_parent(&doc.source);
                            entities.push(entity);
                        }
                    }
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
        let values: Vec<String> = if def.name.starts_with("builtin:") {
            let builtin = dictionaries::get_builtin(&def.name).ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    format!("unknown builtin dictionary: {}", def.name),
                )
            })?;
            builtin.to_vec()
        } else {
            def.values.clone()
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
