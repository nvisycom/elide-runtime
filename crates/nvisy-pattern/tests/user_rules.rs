//! End-to-end: load user-supplied rules from the on-disk wire shape
//! (`testdata/patterns/*.json`, `testdata/dictionaries/*.{json,csv}`)
//! through [`Regex::from_json`], [`Dictionary::metadata_from_json`],
//! and [`Terms::from_csv`], mix them with shipped patterns, and
//! confirm a real internal-handoff document yields the custom
//! entities.

use nvisy_core::{Context, EntityRecognizer, TextData};
use nvisy_ontology::entity::EntityKind;
use nvisy_pattern::recognition::{Dictionary, PatternRecognizer, PatternRegistry, Regex, Terms};
use nvisy_pattern::shipped;

#[tokio::test]
async fn user_json_rules_load_and_detect() {
    let employee_id = Regex::from_json(include_bytes!("../testdata/patterns/employee_id.json"))
        .expect("employee_id.json parses");
    let product_code_regex =
        Regex::from_json(include_bytes!("../testdata/patterns/product_codes.json"))
            .expect("product_codes.json parses");

    let terms = Terms::from_csv(include_bytes!("../testdata/dictionaries/product_codes.csv"))
        .expect("product_codes.csv parses");
    let product_code_dict = Dictionary::metadata_from_json(include_bytes!(
        "../testdata/dictionaries/product_codes.json"
    ))
    .expect("product_codes metadata parses")
    .with_terms(terms)
    .build()
    .expect("dictionary builds");

    // 4 rows × 3 columns; every non-empty cell becomes a term.
    assert_eq!(product_code_dict.terms.len(), 12);

    // Mix user rules with shipped (so the input also sees email etc.).
    let mut registry = PatternRegistry::new()
        .with_pattern(employee_id)
        .with_pattern(product_code_regex)
        .with_dictionary(product_code_dict);
    for p in shipped::patterns::all() {
        registry = registry.with_pattern(p);
    }

    let recognizer = PatternRecognizer::builder()
        .with_registry(registry)
        .build()
        .expect("recognizer builds");

    let text = include_str!("../testdata/inputs/internal.txt");
    let ctx = Context::new(TextData::new(text.to_owned()));
    let entities = recognizer.recognize(&ctx).await.expect("recognize");

    // The custom regex finds both employee numbers.
    let emp_hits: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_kind == EntityKind::InternalId)
        .map(|e| &text[e.location.start..e.location.end])
        .collect();
    assert!(
        emp_hits.contains(&"EMP-12345"),
        "expected EMP-12345 among InternalId hits, got {emp_hits:?}"
    );
    assert!(
        emp_hits.contains(&"EMP-67890"),
        "expected EMP-67890 among InternalId hits, got {emp_hits:?}"
    );

    // Both the user regex and the user dictionary should fire on
    // `WIDGET-200`: regex matches the code, dictionary matches the
    // same code as a literal term.
    assert!(
        emp_hits.contains(&"WIDGET-200"),
        "expected WIDGET-200 among InternalId hits, got {emp_hits:?}"
    );

    // Dictionary fires on the alias term `premium-widget` and the
    // canonical full name `Acme Premium Widget` (substring of "as
    // Acme Premium Widget."). Either is enough to prove the
    // dictionary layer ran.
    assert!(
        emp_hits.contains(&"premium-widget") || emp_hits.contains(&"Acme Premium Widget"),
        "expected dictionary alias/full-name hit, got {emp_hits:?}"
    );

    // Shipped email pattern fires too — proves user + shipped coexist.
    assert!(
        entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::EmailAddress
                && &text[e.location.start..e.location.end] == "counsel@example.com"),
        "expected shipped email pattern to fire alongside user rules"
    );
}
