//! End-to-end tests for `PatternEngineBuilder::with_pattern_dir` and
//! `with_dictionary_dir`: load synthetic patterns + dictionary from
//! `testdata/{patterns,dictionaries}/` and assert the custom patterns
//! fire correctly, individually and combined with built-ins.

use std::path::PathBuf;

use nvisy_ontology::entity::{Entity, EntityKind};
use nvisy_ontology::modality::Text;
use nvisy_pattern::PatternEngine;
use nvisy_pattern::filter::PatternContext;

fn testdata(parts: &[&str]) -> PathBuf {
    let mut path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "testdata"].iter().collect();
    path.extend(parts);
    path
}

fn read_fixture(parts: &[&str]) -> String {
    let path = testdata(parts);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

fn find_entity_at(entities: &[Entity<Text>], start: usize) -> Option<&Entity<Text>> {
    entities.iter().find(|e| e.location.start_offset == start)
}

#[test]
fn with_pattern_dir_loads_custom_regex_pattern() {
    // Scope to the custom regex pattern only — the dict pattern lives
    // in the same dir but references a dictionary we're not loading.
    let engine = PatternEngine::builder()
        .with_pattern_dir(testdata(&["patterns"]))
        .with_patterns(&["internal-employee-id"])
        .build()
        .expect("build engine with custom regex pattern");

    let text = "Engineer EMP-12345 hands off to EMP-67890.";
    let entities = engine.scan_text(text, &PatternContext::default());

    let employee_ids: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_kind == EntityKind::InternalId)
        .collect();
    assert_eq!(
        employee_ids.len(),
        2,
        "expected two employee ids, got {employee_ids:?}",
    );
}

#[test]
fn with_dictionary_dir_resolves_per_column_confidence() {
    let engine = PatternEngine::builder()
        .with_pattern_dir(testdata(&["patterns"]))
        .with_dictionary_dir(testdata(&["dictionaries"]))
        .with_patterns(&["internal-product-codes"])
        .build()
        .expect("build engine with custom dictionary pattern");

    // One sentence per assertion — keeps the text small and the
    // matched-offset lookups unambiguous.
    let text = "WIDGET-200 a.k.a. Acme Premium Widget a.k.a. premium-widget.";
    let entities = engine.scan_text(text, &PatternContext::default());

    // Column 0 (`code`): WIDGET-200 → 0.95
    let code_offset = text.find("WIDGET-200").expect("WIDGET-200 in text");
    let code = find_entity_at(&entities, code_offset)
        .unwrap_or_else(|| panic!("no entity at WIDGET-200; got {entities:?}"));
    assert_eq!(code.entity_kind, EntityKind::InternalId);
    assert!(
        (code.confidence.get() - 0.95).abs() < f64::EPSILON,
        "WIDGET-200 (column 0) should resolve to 0.95, got {}",
        code.confidence.get(),
    );

    // Column 1 (`full_name`): Acme Premium Widget → 0.85
    let full_offset = text
        .find("Acme Premium Widget")
        .expect("full name in text");
    let full = find_entity_at(&entities, full_offset)
        .unwrap_or_else(|| panic!("no entity at full name; got {entities:?}"));
    assert!(
        (full.confidence.get() - 0.85).abs() < f64::EPSILON,
        "Acme Premium Widget (column 1) should resolve to 0.85, got {}",
        full.confidence.get(),
    );

    // Column 2 (`alias`): premium-widget → 0.55. Chosen because no
    // other column carries it case-insensitively, sidestepping the
    // multi-column collision the engine rejects at compile time.
    let alias_offset = text.find("premium-widget").expect("alias in text");
    let alias = find_entity_at(&entities, alias_offset)
        .unwrap_or_else(|| panic!("no entity at alias; got {entities:?}"));
    assert!(
        (alias.confidence.get() - 0.55).abs() < f64::EPSILON,
        "premium-widget (column 2) should resolve to 0.55, got {}",
        alias.confidence.get(),
    );
}

#[test]
fn custom_patterns_coexist_with_builtins() {
    // No with_patterns() filter: extras layer onto the full built-in
    // set, exercising the realistic "extend the engine" deployment.
    let engine = PatternEngine::builder()
        .with_pattern_dir(testdata(&["patterns"]))
        .with_dictionary_dir(testdata(&["dictionaries"]))
        .build()
        .expect("build engine with custom + built-in patterns");

    let text = read_fixture(&["inputs", "internal.txt"]);
    let entities = engine.scan_text(&text, &PatternContext::default());

    let kinds: Vec<_> = entities.iter().map(|e| e.entity_kind).collect();
    let internal_ids = kinds
        .iter()
        .filter(|&&k| k == EntityKind::InternalId)
        .count();
    assert!(
        internal_ids >= 3,
        "expected at least 3 InternalId hits (2 employee ids + product code), got {kinds:?}",
    );
    assert!(
        kinds.contains(&EntityKind::EmailAddress),
        "built-in email pattern should still fire; got {kinds:?}",
    );
}
