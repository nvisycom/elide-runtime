//! Wire → engine tests for caller-inlined custom patterns and
//! dictionaries on [`RecognizerParams`], plus the two guardrails
//! currently enforced end-to-end (source length, rule count).
//! See #317 for the automaton-size follow-up.

use bytes::Bytes;
use elide_core::entity::LabelRef;
use elide_core::primitive::Confidence;
use nvisy_engine::{Engine, EntityGroup};
use nvisy_schema::file::Document;
use nvisy_schema::plan::{
    AnalyzerParams, CustomDictionary, CustomDictionaryTerm, CustomPatternContext,
    CustomPatternRule, CustomPatternVariant, MAX_REGEX_SOURCE_LEN, RecognizerParams,
};

const SAMPLE_DOCX: &[u8] = include_bytes!("testdata/sample.docx");

fn raw_docx() -> Document {
    Document::new(Bytes::from_static(SAMPLE_DOCX), "docx")
}

fn engine() -> Engine {
    Engine::new()
}

fn spec_with_recognizers(recognizers: RecognizerParams) -> AnalyzerParams {
    AnalyzerParams {
        recognizers,
        ..Default::default()
    }
}

fn one_variant(regex: &str) -> CustomPatternVariant {
    CustomPatternVariant {
        regex: regex.to_owned(),
        score: Confidence::MAX,
        validator: None,
    }
}

fn one_rule(name: &str, label: &str, regex: &str) -> CustomPatternRule {
    CustomPatternRule {
        name: name.to_owned(),
        label: LabelRef::new(label),
        variants: vec![one_variant(regex)],
        context: CustomPatternContext::default(),
        languages: Vec::new(),
        countries: Vec::new(),
    }
}

#[tokio::test]
async fn custom_regex_produces_entity_with_caller_label() {
    let engine = engine();
    let spec = spec_with_recognizers(RecognizerParams {
        custom: vec![one_rule("alice_rule", "alice_id", r"\balice\b")],
        custom_dictionaries: Vec::new(),
    });

    let analyzed = engine
        .analyze(raw_docx(), &[], &spec)
        .await
        .expect("analyze succeeds");

    let body = analyzed.body.as_ref().expect("body group present");
    let EntityGroup::Text(entities) = body else {
        panic!("expected Text body, got {body:?}");
    };
    let alice = LabelRef::new("alice_id");
    assert!(
        entities.iter().any(|e| e.entity.label == alice),
        "expected at least one entity with label alice_id; got labels: {:?}",
        entities.iter().map(|e| &e.entity.label).collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn custom_dictionary_produces_entity_with_caller_label() {
    let engine = engine();
    let spec = spec_with_recognizers(RecognizerParams {
        custom: Vec::new(),
        custom_dictionaries: vec![CustomDictionary {
            name: "greetings".to_owned(),
            label: LabelRef::new("greeting"),
            terms: vec![CustomDictionaryTerm {
                term: "alice".to_owned(),
                score: None,
            }],
            score: Confidence::MAX,
            context: CustomPatternContext::default(),
            languages: Vec::new(),
            countries: Vec::new(),
        }],
    });

    let analyzed = engine
        .analyze(raw_docx(), &[], &spec)
        .await
        .expect("analyze succeeds");

    let body = analyzed.body.as_ref().expect("body group present");
    let EntityGroup::Text(entities) = body else {
        panic!("expected Text body, got {body:?}");
    };
    let greeting = LabelRef::new("greeting");
    assert!(
        entities.iter().any(|e| e.entity.label == greeting),
        "expected at least one entity with label greeting; got labels: {:?}",
        entities.iter().map(|e| &e.entity.label).collect::<Vec<_>>(),
    );
}

#[test]
fn overlong_regex_source_rejects_at_deserialize() {
    let src = "a".repeat(MAX_REGEX_SOURCE_LEN + 1);
    let json = serde_json::json!({
        "regex": src,
        "score": 1.0,
    });
    let err = serde_json::from_value::<CustomPatternVariant>(json)
        .expect_err("regex over the cap must reject at deserialize");
    let msg = err.to_string();
    assert!(
        msg.contains("regex source too long"),
        "expected `regex source too long` error, got: {msg}",
    );
    assert!(
        msg.contains(&MAX_REGEX_SOURCE_LEN.to_string()),
        "expected the cap value in the error, got: {msg}",
    );
}

#[tokio::test]
async fn too_many_custom_rules_rejects_at_analyze() {
    let engine = engine();
    let rules: Vec<CustomPatternRule> = (0..33)
        .map(|i| one_rule(&format!("rule_{i}"), "custom", r"\balice\b"))
        .collect();
    let spec = spec_with_recognizers(RecognizerParams {
        custom: rules,
        custom_dictionaries: Vec::new(),
    });

    let err = engine
        .analyze(raw_docx(), &[], &spec)
        .await
        .expect_err("33 rules must exceed the per-request cap");
    let msg = err.to_string();
    assert!(
        msg.contains("exceeds the per-request cap"),
        "expected `exceeds the per-request cap` error, got: {msg}",
    );
}
