//! End-to-end test for `PatternEngineBuilder::with_pattern_dir` +
//! `with_dictionary_dir`: load synthetic patterns + dictionary from
//! `testdata/{patterns,dictionaries}/`, scan
//! `testdata/inputs/internal.txt`, and assert that the custom
//! patterns fire (with the right per-column confidence) and that the
//! built-in patterns still apply alongside them.

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
fn extension_engine_loads_custom_patterns_and_dictionary() {
    let engine = PatternEngine::builder()
        .with_pattern_dir(testdata(&["patterns"]))
        .with_dictionary_dir(testdata(&["dictionaries"]))
        .build()
        .expect("build extension engine");

    let text = read_fixture(&["inputs", "internal.txt"]);
    let entities = engine.scan_text(&text, &PatternContext::default());

    // Custom regex pattern: matches every EMP-#####.
    let employee_ids: Vec<_> = entities
        .iter()
        .filter(|e| {
            e.entity_kind == EntityKind::InternalId
                && text[e.location.start_offset..e.location.end_offset].starts_with("EMP-")
        })
        .collect();
    assert_eq!(
        employee_ids.len(),
        2,
        "expected two employee ids, got {employee_ids:?}",
    );

    // Custom dictionary pattern, column 0 (`code`): WIDGET-200 → 0.95.
    let widget_offset = text
        .find("WIDGET-200")
        .expect("fixture contains WIDGET-200");
    let widget = find_entity_at(&entities, widget_offset)
        .unwrap_or_else(|| panic!("no entity at WIDGET-200 offset; got {entities:?}"));
    assert_eq!(widget.entity_kind, EntityKind::InternalId);
    assert!(
        (widget.confidence.get() - 0.95).abs() < f64::EPSILON,
        "WIDGET-200 (column 0) should map to confidence 0.95, got {}",
        widget.confidence.get(),
    );

    // Custom dictionary pattern, column 1 (`full_name`):
    // `Acme Premium Widget` → 0.85.
    let full_name_offset = text
        .find("Acme Premium Widget")
        .expect("fixture contains the full product name");
    let full_name = find_entity_at(&entities, full_name_offset)
        .unwrap_or_else(|| panic!("no entity at full-name offset; got {entities:?}"));
    assert!(
        (full_name.confidence.get() - 0.85).abs() < f64::EPSILON,
        "Acme Premium Widget (column 1) should map to confidence 0.85, got {}",
        full_name.confidence.get(),
    );

    // Custom dictionary pattern, column 2 (`alias`): `premium-widget`
    // is uniquely a column-2 term — no other column carries it
    // (case-insensitively), so it cleanly tests the alias confidence.
    let alias_offset = text
        .find("premium-widget")
        .expect("fixture contains the alias");
    let alias = find_entity_at(&entities, alias_offset)
        .unwrap_or_else(|| panic!("no entity at premium-widget offset; got {entities:?}"));
    assert!(
        (alias.confidence.get() - 0.55).abs() < f64::EPSILON,
        "premium-widget (column 2) should map to confidence 0.55, got {}",
        alias.confidence.get(),
    );

    // Built-in patterns stay active alongside the extras.
    assert!(
        entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::EmailAddress),
        "built-in email pattern should still fire; got {entities:?}",
    );
}
