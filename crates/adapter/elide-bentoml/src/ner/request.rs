//! Convert Rust domain requests into the Python-owned wire contract.

use std::collections::HashSet;

use elide_core::primitive::LanguageTag;
use elide_ner::backend::NerRequest;

use super::wire::{EntitySpec, NerRequest as WireNerRequest, Schema};
use crate::error::BentoError;

impl WireNerRequest {
    /// This schema-driven backend requires explicit, nonempty entity labels.
    pub(super) fn from_request(
        request: &NerRequest<'_>,
        default_threshold: f32,
    ) -> Result<Self, BentoError> {
        if request.text.is_empty() {
            return Err(BentoError::Protocol("NER text must not be empty".into()));
        }
        if !(0.0..=1.0).contains(&default_threshold) {
            return Err(BentoError::Protocol(
                "NER threshold must be in [0, 1]".into(),
            ));
        }
        let labels = request.labels.ok_or_else(|| {
            BentoError::Protocol("schema-driven NER requires explicit entity labels".into())
        })?;
        if labels.is_empty() {
            return Err(BentoError::Protocol(
                "NER schema must contain entity labels".into(),
            ));
        }
        let english = LanguageTag::english();
        let language = request.language.unwrap_or(&english);
        let mut seen = HashSet::new();
        let mut entities = Vec::with_capacity(labels.len());
        for label in labels {
            let name = label.name(language);
            if name.is_empty() || !seen.insert(name) {
                return Err(BentoError::Protocol(
                    "NER entity labels must be nonempty and unique".into(),
                ));
            }
            entities.push(EntitySpec {
                label: super::wire::Label::try_from(name)
                    .map_err(|error| BentoError::Protocol(error.to_string()))?,
                description: label.description(language).map(str::to_owned),
                threshold: None,
            });
        }
        Ok(Self {
            text: super::wire::Text::try_from(request.text)
                .map_err(|error| BentoError::Protocol(error.to_string()))?,
            schema: Schema { entities },
            threshold: Some(f64::from(default_threshold)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_labels_are_rejected_locally() {
        let request = NerRequest {
            text: "Ada",
            labels: None,
            language: None,
            correlation_id: None,
        };
        assert!(WireNerRequest::from_request(&request, 0.5).is_err());
    }
}

#[cfg(test)]
mod contract_tests {
    use elide_core::entity::{Label, LabelLocale};

    use super::*;
    use crate::ner::wire::NerInput;

    #[test]
    fn domain_requests_serialize_to_python_fixtures() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../contracts/ner/v1/fixtures.json"
        ))
        .unwrap();
        for case in fixtures.as_array().unwrap() {
            let mut expected = case["request"].clone();
            let request = &expected["requests"][0];
            let text = request["text"].as_str().unwrap().to_owned();
            let labels: Vec<Label> = request["schema"]["entities"]
                .as_array()
                .unwrap()
                .iter()
                .map(|spec| {
                    let name = spec["label"].as_str().unwrap().to_owned();
                    let mut label = Label::new(name.clone(), name.clone());
                    if let Some(description) = spec["description"].as_str() {
                        label = label.with_localization(
                            LanguageTag::english(),
                            LabelLocale::described(name, description.to_owned()),
                        );
                    }
                    label
                })
                .collect();
            let domain = NerRequest {
                text: &text,
                labels: Some(&labels),
                language: None,
                correlation_id: None,
            };
            let wire = NerInput {
                requests: vec![WireNerRequest::from_request(&domain, 0.5).unwrap()],
            };
            for spec in expected["requests"][0]["schema"]["entities"]
                .as_array_mut()
                .unwrap()
            {
                spec.as_object_mut()
                    .unwrap()
                    .retain(|_, value| !value.is_null());
            }
            assert_eq!(serde_json::to_value(wire).unwrap(), expected);
        }
    }

    #[test]
    fn invalid_requests_are_rejected_before_transport() {
        let duplicate_labels = vec![Label::new("a", "person"), Label::new("b", "person")];
        let labels = vec![Label::new("a", "person")];
        let empty_label = vec![Label::new("a", "")];
        for (text, labels, threshold) in [
            ("Ada", &duplicate_labels, 0.5),
            ("Ada", &empty_label, 0.5),
            ("", &labels, 0.5),
            ("Ada", &labels, f32::NAN),
            ("Ada", &labels, 1.1),
        ] {
            let request = NerRequest {
                text,
                labels: Some(labels),
                language: None,
                correlation_id: None,
            };
            assert!(WireNerRequest::from_request(&request, threshold).is_err());
        }
    }
}
