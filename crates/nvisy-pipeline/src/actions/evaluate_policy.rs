//! Policy evaluation action that maps detected entities to redaction instructions.

use serde::Deserialize;

use nvisy_ontology::entity::Entity;
use nvisy_ontology::policy::PolicyRule;
use nvisy_ontology::redaction::{
    AudioRedactionOutput, AudioRedactionSpec, ImageRedactionOutput, ImageRedactionSpec, Redaction,
    RedactionOutput, RedactionSpec, TextRedactionOutput, TextRedactionSpec,
};
use nvisy_core::error::Error;

use crate::action::Action;

/// Typed parameters for [`EvaluatePolicyAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatePolicyParams {
    /// Ordered policy rules to evaluate.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
    /// Fallback redaction specification when no rule matches.
    #[serde(default = "default_spec")]
    pub default_spec: RedactionSpec,
    /// Fallback confidence threshold.
    #[serde(default = "default_threshold")]
    pub default_confidence_threshold: f64,
}

fn default_spec() -> RedactionSpec {
    RedactionSpec::Text(TextRedactionSpec::Mask { mask_char: '*' })
}
fn default_threshold() -> f64 {
    0.5
}

/// Evaluates policy rules against detected entities and produces [`Redaction`] instructions.
///
/// For each entity the action finds the first matching rule (sorted by priority),
/// applies its redaction spec and replacement template, and creates a
/// [`Redaction`]. Entities that fall below the confidence threshold are skipped.
pub struct EvaluatePolicyAction {
    params: EvaluatePolicyParams,
}

#[async_trait::async_trait]
impl Action for EvaluatePolicyAction {
    type Params = EvaluatePolicyParams;
    type Input = Vec<Entity>;
    type Output = Vec<Redaction>;

    fn id(&self) -> &str {
        "evaluate-policy"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        entities: Self::Input,
    ) -> Result<Vec<Redaction>, Error> {
        let default_spec = &self.params.default_spec;
        let default_threshold = self.params.default_confidence_threshold;

        let mut sorted_rules = self.params.rules.clone();
        sorted_rules.sort_by_key(|r| r.priority);

        let mut redactions = Vec::new();

        for entity in &entities {
            let rule = find_matching_rule(entity, &sorted_rules);
            let spec = rule.map(|r| &r.spec).unwrap_or(default_spec);

            if rule.is_none() && entity.confidence < default_threshold {
                continue;
            }

            let output = if let Some(r) = rule {
                build_output_from_template(spec, &r.replacement_template, entity)
            } else {
                build_default_output(entity, spec)
            };

            let mut redaction = Redaction::new(entity.source.as_uuid(), output);
            redaction = redaction.with_original_value(&entity.value);
            if let Some(r) = rule {
                redaction = redaction.with_policy_rule_id(r.id);
            }
            redaction.source.set_parent_id(Some(entity.source.as_uuid()));

            redactions.push(redaction);
        }

        Ok(redactions)
    }
}

/// Returns the first enabled rule whose [`EntitySelector`] matches the given entity,
/// or `None` if no rule applies.
fn find_matching_rule<'a>(entity: &Entity, rules: &'a [PolicyRule]) -> Option<&'a PolicyRule> {
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if rule.selector.matches(&entity.category, &entity.entity_type, entity.confidence) {
            return Some(rule);
        }
    }
    None
}

/// Expands a replacement template using entity metadata.
///
/// Supported placeholders: `{entityType}`, `{category}`, `{value}`.
fn apply_template(template: &str, entity: &Entity) -> String {
    template
        .replace("{entityType}", &entity.entity_type)
        .replace(
            "{category}",
            &format!("{:?}", entity.category).to_lowercase(),
        )
        .replace("{value}", &entity.value)
}

/// Builds a [`RedactionOutput`] from a spec and a policy rule's replacement template.
fn build_output_from_template(
    spec: &RedactionSpec,
    template: &str,
    entity: &Entity,
) -> RedactionOutput {
    let replacement = apply_template(template, entity);
    build_output_with_replacement(spec, replacement)
}

/// Generates a [`RedactionOutput`] for an entity using the given default redaction spec.
fn build_default_output(entity: &Entity, spec: &RedactionSpec) -> RedactionOutput {
    match spec {
        RedactionSpec::Text(text) => {
            let replacement = match text {
                TextRedactionSpec::Mask { mask_char } => {
                    mask_char.to_string().repeat(entity.value.len())
                }
                TextRedactionSpec::Replace { placeholder } => {
                    if placeholder.is_empty() {
                        format!("[{}]", entity.entity_type.to_uppercase())
                    } else {
                        apply_template(placeholder, entity)
                    }
                }
                TextRedactionSpec::Remove => String::new(),
                TextRedactionSpec::Hash => format!("[HASH:{}]", entity.entity_type),
                TextRedactionSpec::Encrypt { .. } => format!("[ENC:{}]", entity.entity_type),
                TextRedactionSpec::Synthesize => format!("[SYNTH:{}]", entity.entity_type),
                TextRedactionSpec::Pseudonymize => format!("[PSEUDO:{}]", entity.entity_type),
                TextRedactionSpec::Tokenize { .. } => format!("[TOKEN:{}]", entity.entity_type),
                TextRedactionSpec::Aggregate => format!("[AGG:{}]", entity.entity_type),
                TextRedactionSpec::Generalize { .. } => format!("[GEN:{}]", entity.entity_type),
                TextRedactionSpec::DateShift { .. } => format!("[SHIFTED:{}]", entity.entity_type),
            };
            build_output_with_replacement(spec, replacement)
        }
        RedactionSpec::Image(img) => RedactionOutput::Image(match img {
            ImageRedactionSpec::Blur { sigma } => ImageRedactionOutput::Blur { sigma: *sigma },
            ImageRedactionSpec::Block { color } => ImageRedactionOutput::Block { color: *color },
            ImageRedactionSpec::Pixelate { block_size } => {
                ImageRedactionOutput::Pixelate { block_size: *block_size }
            }
            ImageRedactionSpec::Synthesize => ImageRedactionOutput::Synthesize,
        }),
        RedactionSpec::Audio(audio) => RedactionOutput::Audio(match audio {
            AudioRedactionSpec::Silence => AudioRedactionOutput::Silence,
            AudioRedactionSpec::Remove => AudioRedactionOutput::Remove,
            AudioRedactionSpec::Synthesize => AudioRedactionOutput::Synthesize,
        }),
    }
}

/// Builds a [`RedactionOutput`] from a spec and a replacement string.
fn build_output_with_replacement(spec: &RedactionSpec, replacement: String) -> RedactionOutput {
    match spec {
        RedactionSpec::Text(text) => RedactionOutput::Text(match text {
            TextRedactionSpec::Mask { mask_char } => TextRedactionOutput::Mask {
                replacement,
                mask_char: *mask_char,
            },
            TextRedactionSpec::Replace { .. } => TextRedactionOutput::Replace { replacement },
            TextRedactionSpec::Hash => TextRedactionOutput::Hash {
                hash_value: replacement,
            },
            TextRedactionSpec::Encrypt { key_id } => TextRedactionOutput::Encrypt {
                ciphertext: replacement,
                key_id: key_id.clone(),
            },
            TextRedactionSpec::Remove => TextRedactionOutput::Remove,
            TextRedactionSpec::Synthesize => TextRedactionOutput::Synthesize { replacement },
            TextRedactionSpec::Pseudonymize => TextRedactionOutput::Pseudonymize {
                pseudonym: replacement,
            },
            TextRedactionSpec::Tokenize { vault_id } => TextRedactionOutput::Tokenize {
                token: replacement,
                vault_id: vault_id.clone(),
            },
            TextRedactionSpec::Aggregate => TextRedactionOutput::Aggregate { replacement },
            TextRedactionSpec::Generalize { level } => TextRedactionOutput::Generalize {
                replacement,
                level: *level,
            },
            TextRedactionSpec::DateShift { offset_days } => TextRedactionOutput::DateShift {
                replacement,
                offset_days: *offset_days,
            },
        }),
        RedactionSpec::Image(img) => RedactionOutput::Image(match img {
            ImageRedactionSpec::Blur { sigma } => ImageRedactionOutput::Blur { sigma: *sigma },
            ImageRedactionSpec::Block { color } => ImageRedactionOutput::Block { color: *color },
            ImageRedactionSpec::Pixelate { block_size } => {
                ImageRedactionOutput::Pixelate { block_size: *block_size }
            }
            ImageRedactionSpec::Synthesize => ImageRedactionOutput::Synthesize,
        }),
        RedactionSpec::Audio(audio) => RedactionOutput::Audio(match audio {
            AudioRedactionSpec::Silence => AudioRedactionOutput::Silence,
            AudioRedactionSpec::Remove => AudioRedactionOutput::Remove,
            AudioRedactionSpec::Synthesize => AudioRedactionOutput::Synthesize,
        }),
    }
}
