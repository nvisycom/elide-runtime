//! Resolving a lineup: what an engine's [`Availability`] permits,
//! what a request's [`ComponentSelection`] narrows to, and which
//! combinations are refused rather than quietly narrowed.

use elide::ErrorKind;
use elide_provider::{
    Availability, Component, ComponentSelection, Enrichers, NerBackend, ProviderConfig, Recognizers,
};

/// A lineup of three NER components in two families.
fn lineup() -> Recognizers {
    Recognizers {
        ner: vec![
            ner("general-pii", &["general"]),
            ner("medical-ner", &["medical", "advanced"]),
            ner("finance-ner", &["finance", "advanced"]),
        ],
        llm: Vec::new(),
    }
}

fn ner(name: &str, tags: &[&str]) -> Component<NerBackend> {
    Component {
        name: name.into(),
        tags: tags.iter().map(|t| (*t).into()).collect(),
        backend: NerBackend::Mock,
    }
}

/// Build a provider over `lineup()` restricted to `availability`,
/// then resolve `selection` against it, returning the names that
/// would run.
fn resolved(
    availability: Availability,
    selection: &ComponentSelection,
) -> Result<Vec<String>, (ErrorKind, String)> {
    let provider = ProviderConfig {
        recognizers: lineup(),
        enrichers: Enrichers::default(),
    }
    .build()
    .restricted_to(availability);

    provider
        .resolved_components(selection)
        .map(|r| r.ner)
        .map_err(|err| (err.kind(), err.to_string()))
}

/// The default: everything registered runs.
#[test]
fn unrestricted_and_unselected_runs_everything() {
    let names = resolved(Availability::All, &ComponentSelection::new()).expect("resolves");
    assert_eq!(names, ["general-pii", "medical-ner", "finance-ner"]);
}

/// A selection naming nothing must mean *everything*, not nothing.
/// The opposite would let an omitted field silently disable
/// detection.
#[test]
fn an_empty_selection_means_every_available_component() {
    let names = resolved(
        Availability::Only(vec!["general".into()]),
        &ComponentSelection::new(),
    )
    .expect("resolves");
    assert_eq!(names, ["general-pii"]);
}

/// Availability and selection match a tag as readily as a name, so
/// a family is named once rather than enumerated.
#[test]
fn a_tag_selects_every_component_carrying_it() {
    let names = resolved(
        Availability::All,
        &ComponentSelection::new().with_only(["advanced"]),
    )
    .expect("resolves");
    assert_eq!(names, ["medical-ner", "finance-ner"]);
}

/// `skip` applies after `only`, so a caller runs everything but one
/// component without listing the rest.
#[test]
fn skip_narrows_what_only_selected() {
    let names = resolved(
        Availability::All,
        &ComponentSelection::new().with_skip(["finance-ner"]),
    )
    .expect("resolves");
    assert_eq!(names, ["general-pii", "medical-ner"]);
}

/// `Except` leaves a later-added component available, which is why
/// it exists beside `Only`.
#[test]
fn except_withholds_only_what_it_names() {
    let names = resolved(
        Availability::Except(vec!["medical".into()]),
        &ComponentSelection::new(),
    )
    .expect("resolves");
    assert_eq!(names, ["general-pii", "finance-ner"]);
}

/// The load-bearing refusal: asking for a component the caller may
/// not use is an error, not a smaller lineup. A caller that
/// silently got weaker detection could not tell that result from a
/// clean document.
#[test]
fn selecting_an_unavailable_component_is_refused() {
    let (kind, message) = resolved(
        Availability::Only(vec!["general".into()]),
        &ComponentSelection::new().with_only(["medical-ner"]),
    )
    .expect_err("must refuse");

    assert_eq!(kind, ErrorKind::CapabilityUnavailable);
    assert!(
        message.contains("medical-ner"),
        "the error must name what was refused; got: {message}",
    );
}

/// A key matching nothing registered is a typo. Without this it
/// would silently mean "no filtering", which is the dangerous
/// reading.
#[test]
fn selecting_an_unknown_component_is_refused() {
    let (kind, message) = resolved(
        Availability::All,
        &ComponentSelection::new().with_only(["medcial-ner"]),
    )
    .expect_err("must refuse");

    assert_eq!(kind, ErrorKind::Configuration);
    assert!(
        message.contains("medcial-ner"),
        "the error must name the unknown key; got: {message}",
    );
}

/// Narrowing everything away is a mistake, not a request for an
/// empty report.
#[test]
fn selecting_every_component_away_is_refused() {
    let (kind, _) = resolved(
        Availability::All,
        &ComponentSelection::new().with_skip(["general", "advanced"]),
    )
    .expect_err("must refuse");

    assert_eq!(kind, ErrorKind::Configuration);
}

/// A deployment offering several OCR engines is legitimate; the cap
/// is on what a request resolves to, not on what is configured.
mod enrichers {
    use elide::ErrorKind;
    use elide_provider::{
        Availability, Component, ComponentSelection, Enrichers, OcrBackend, ProviderConfig,
        Recognizers,
    };

    fn ocr(name: &str, tags: &[&str]) -> Component<OcrBackend> {
        Component {
            name: name.into(),
            tags: tags.iter().map(|t| (*t).into()).collect(),
            backend: OcrBackend::Mock,
        }
    }

    /// Three OCR engines offered, none selected by default.
    fn provider(availability: Availability) -> elide_provider::Provider {
        ProviderConfig {
            recognizers: Recognizers::default(),
            enrichers: Enrichers {
                ocr: vec![
                    ocr("fast-ocr", &["fast"]),
                    ocr("accurate-ocr", &["accurate"]),
                    ocr("handwriting-ocr", &["accurate", "handwriting"]),
                ],
                stt: Vec::new(),
            },
        }
        .build()
        .restricted_to(availability)
    }

    /// A request picks one of the offered engines by name.
    #[test]
    fn a_request_selects_one_enricher_from_several() {
        let resolved = provider(Availability::All)
            .resolved_components(&ComponentSelection::new().with_only(["accurate-ocr"]))
            .expect("resolves");
        assert_eq!(resolved.ocr, ["accurate-ocr"]);
    }

    /// Leaving several resolved is ambiguous: elide attaches at
    /// most one per analyzer, so the request must narrow rather
    /// than have one picked for it.
    #[test]
    fn several_resolved_enrichers_are_refused_at_compile() {
        let provider = provider(Availability::All);
        let selection = ComponentSelection::new().with_only(["accurate"]);

        // Two carry the "accurate" tag, so inspection reports both…
        let resolved = provider
            .resolved_components(&selection)
            .expect("inspection reports the ambiguity");
        assert_eq!(resolved.ocr, ["accurate-ocr", "handwriting-ocr"]);

        // …and compiling the request refuses it, naming the choice.
        // A policy naming something to find: the empty-catalog
        // guard refuses first otherwise, and would mask this.
        let policy = elide_governance::policy::Policy {
            id: uuid::Uuid::now_v7(),
            name: "detect".into(),
            scopes: vec![elide_governance::policy::LabelScope::new(
                "contact",
                vec![elide::entity::LabelRef::new("email_address")],
            )],
            ..Default::default()
        };
        let err = match provider.analyze_orchestrator(
            &Default::default(),
            &[],
            std::slice::from_ref(&policy),
            uuid::Uuid::now_v7(),
            &selection,
        ) {
            Ok(_) => panic!("two resolved enrichers must be refused"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), ErrorKind::Configuration);
        let message = err.to_string();
        assert!(
            message.contains("accurate-ocr") && message.contains("handwriting-ocr"),
            "the error must name the candidates to narrow between; got: {message}",
        );
    }

    /// An availability that leaves exactly one engine reachable
    /// needs no selection at all.
    #[test]
    fn availability_alone_can_settle_the_choice() {
        let resolved = provider(Availability::Only(vec!["fast".into()]))
            .resolved_components(&ComponentSelection::new())
            .expect("resolves");
        assert_eq!(resolved.ocr, ["fast-ocr"]);
    }

    /// Skipping every enricher turns it off for this document,
    /// which is legitimate — unlike narrowing every recognizer
    /// away, an absent enricher means the modality is not enriched,
    /// not that nothing can be detected.
    #[test]
    fn skipping_every_enricher_is_allowed() {
        let resolved = provider(Availability::All)
            .resolved_components(&ComponentSelection::new().with_skip(["fast", "accurate"]))
            .expect("resolves");
        assert!(resolved.ocr.is_empty());
    }
}
