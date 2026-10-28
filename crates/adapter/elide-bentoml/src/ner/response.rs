//! Adapt Python Unicode code-point offsets to Rust UTF-8 byte ranges.

use elide_core::primitive::LanguageTag;
use elide_ner::backend::{NerRequest, NerResponse, NerSpan};

use super::wire::NerResponse as WireNerResponse;
use crate::error::BentoError;

impl WireNerResponse {
    /// Resolve the localized wire names against this request's canonical labels.
    pub(super) fn decode_request(
        self,
        request: &NerRequest<'_>,
    ) -> Result<NerResponse, BentoError> {
        let english = LanguageTag::english();
        let language = request.language.unwrap_or(&english);
        let labels = request.labels.ok_or_else(|| {
            BentoError::Protocol("NER response requires the request's label schema".into())
        })?;
        let mut response = self.decode(request.text)?;
        for span in &mut response.spans {
            let label = labels
                .iter()
                .find(|label| label.name(language) == span.label.as_str())
                .ok_or_else(|| {
                    BentoError::Protocol(format!(
                        "NER returned an unrequested label: {}",
                        span.label.as_str()
                    ))
                })?;
            span.label = label.to_ref();
        }
        Ok(response)
    }

    pub(super) fn decode(self, source: &str) -> Result<NerResponse, BentoError> {
        // Build one table per source, including the valid exclusive end.
        let boundaries: Vec<usize> = source
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(source.len()))
            .collect();
        let mut spans = Vec::with_capacity(self.entities.len());
        for entity in self.entities {
            // Preserve the defensive guard for empty/inverted spans.
            if entity.end <= entity.start {
                continue;
            }
            let start = usize::try_from(entity.start)
                .ok()
                .and_then(|i| boundaries.get(i))
                .copied();
            let end = usize::try_from(entity.end)
                .ok()
                .and_then(|i| boundaries.get(i))
                .copied();
            let (Some(start), Some(end)) = (start, end) else {
                return Err(BentoError::Protocol(
                    "NER span is outside the source text".into(),
                ));
            };
            if source[start..end] != entity.text {
                return Err(BentoError::Protocol(
                    "NER span text does not match the source".into(),
                ));
            }
            if !(0.0..=1.0).contains(&entity.score) {
                return Err(BentoError::Protocol("NER score must be in [0, 1]".into()));
            }
            spans.push(NerSpan::new(entity.label, entity.score as f32, start..end));
        }
        Ok(NerResponse::new(spans))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ner::wire::{NerInput, NerOutput};

    const FIXTURES: &str = include_str!("../../../../../contracts/ner/v1/fixtures.json");

    #[test]
    fn python_fixtures_preserve_unicode_spans_and_wire_shapes() {
        let fixtures: serde_json::Value = serde_json::from_str(FIXTURES).unwrap();
        for case in fixtures.as_array().unwrap() {
            let request: NerInput = serde_json::from_value(case["request"].clone()).unwrap();
            let serialized = serde_json::to_value(&request).unwrap();
            // Optional null fields may be omitted by Rust.
            let roundtrip: NerInput = serde_json::from_value(serialized).unwrap();
            assert_eq!(request.requests[0].text, roundtrip.requests[0].text);
            let mut responses: NerOutput =
                serde_json::from_value(case["response"].clone()).unwrap();
            let decoded = responses
                .0
                .remove(0)
                .decode(&request.requests[0].text)
                .unwrap();
            let expected = case["expected_byte_range"].as_array().unwrap();
            assert_eq!(
                decoded.spans[0].offset,
                expected[0].as_u64().unwrap() as usize..expected[1].as_u64().unwrap() as usize
            );
        }
    }

    #[test]
    fn response_names_resolve_to_canonical_ids_in_the_request_language() {
        use elide_core::entity::{Label, LabelLocale};

        for language in [None, Some(LanguageTag::english())] {
            let labels = [Label::new("person_name", "person name")];
            let request = NerRequest {
                text: "Ada",
                labels: Some(&labels),
                language: language.as_ref(),
                correlation_id: None,
            };
            let wire: WireNerResponse = serde_json::from_str(
                r#"{"model_id":"m","entities":[{"text":"Ada","label":"person name","score":0.9,"start":0,"end":3}]}"#,
            ).unwrap();
            let response = wire.decode_request(&request).unwrap();
            assert_eq!(response.spans[0].label, labels[0].to_ref());
            assert_eq!(response.spans[0].offset, 0..3);
        }
        let language: LanguageTag = "pl".parse().unwrap();
        let labels = [Label::new("person_name", "person name")
            .with_localization(language.clone(), LabelLocale::new("osoba"))];
        let request = NerRequest {
            text: "Ada",
            labels: Some(&labels),
            language: Some(&language),
            correlation_id: None,
        };
        let wire: WireNerResponse = serde_json::from_str(
            r#"{"model_id":"m","entities":[{"text":"Ada","label":"osoba","score":0.9,"start":0,"end":3}]}"#,
        ).unwrap();
        assert_eq!(
            wire.decode_request(&request).unwrap().spans[0].label,
            labels[0].to_ref()
        );
        let wire: WireNerResponse = serde_json::from_str(
            r#"{"model_id":"m","entities":[{"text":"Ada","label":"unknown","score":0.9,"start":0,"end":3}]}"#,
        ).unwrap();
        assert!(wire.decode_request(&request).is_err());
    }

    #[test]
    fn rejects_out_of_bounds_and_mismatched_text() {
        for entity in [
            r#"{"text":"Ada","label":"person","score":0.9,"start":0,"end":99}"#,
            r#"{"text":"Bob","label":"person","score":0.9,"start":0,"end":3}"#,
        ] {
            let response: WireNerResponse =
                serde_json::from_str(&format!(r#"{{"entities":[{entity}],"model_id":"m"}}"#))
                    .unwrap();
            assert!(response.decode("Ada").is_err());
        }
    }

    #[test]
    fn accepts_missing_tokens_and_drops_non_positive_spans() {
        let response: WireNerResponse = serde_json::from_str(
            r#"{
            "model_id":"m", "entities":[
                {"text":"Ada","label":"person","score":0.9,"start":0,"end":3},
                {"text":"","label":"person","score":0.9,"start":2,"end":2}
            ]}"#,
        )
        .unwrap();
        assert!(response.tokens.is_none());
        assert_eq!(response.decode("Ada").unwrap().spans.len(), 1);
    }
}
