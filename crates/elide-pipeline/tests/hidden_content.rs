//! Hidden content end to end: analyze lists every metadata field and
//! collaboration item, marked sensitive or not; a reviewer removes,
//! keeps, or removes all; anonymize does exactly what the preview
//! said.
//!
//! Every test goes through a serialized audit between the passes,
//! the path a stateless host takes, so the flagged items and the
//! reviewer's decisions have to survive it.

use std::io::Read;

use bytes::Bytes;
use elide::entity::LabelRef;
use elide::modality::text::Text;
use elide_pipeline::entity::{
    Edit, EditError, EditSet, HiddenChoice, HiddenItem, Removal, RemoveAll, Suppress,
};
use elide_pipeline::file::Document;
use elide_pipeline::provider::{Deployment, Provider, RequestContext};
use elide_pipeline::{Audit, Engine};
use elide_schema::policy::{LabelScope, Policy};

/// The reviewer every edit is applied as.
const REVIEWER: &str = "reviewer@example.com";

/// A page carrying three hidden items: an author `<meta>` naming a
/// person, a description `<meta>` holding an address, and a
/// reviewer's HTML comment.
const PAGE: &[u8] = b"<html><head>\
    <meta name=\"author\" content=\"Ada Lovelace\">\
    <meta name=\"description\" content=\"write to ada@example.com\">\
    <title>Plan</title></head>\
    <body><!-- reviewer note: call back --><p>Hello</p></body></html>";

/// A DOCX whose core properties name its last editor and carry its
/// created and modified dates, and whose extended properties name a
/// template.
const PROPERTIES_DOCX: &[u8] = include_bytes!("testdata/properties.docx");

/// The last editor `PROPERTIES_DOCX` names.
const EDITOR: &str = "Ada Lovelace";

/// When `PROPERTIES_DOCX` says it was created.
const CREATED: &str = "2021-11-24T18:48:00";

fn engine() -> Engine {
    Engine::new(Provider::new(Deployment::default()))
}

fn page() -> Document {
    Document::new("page.html", Bytes::from_static(PAGE))
}

fn docx() -> Document {
    Document::new("plan.docx", Bytes::from_static(PROPERTIES_DOCX))
}

/// A policy whose catalog is `labels` and that redacts nothing by
/// rule: what happens to hidden content is the plan's alone.
fn catalog(labels: &[&str]) -> Policy {
    Policy {
        id: uuid::Uuid::now_v7(),
        name: "hidden".into(),
        scopes: vec![LabelScope::new(
            "sensitive",
            labels.iter().map(|label| LabelRef::new(*label)),
        )],
        ..Policy::default()
    }
}

/// Analyze `document`, apply what `review` records, and serialize
/// the audit and read it back, as a host stores one between passes.
async fn reviewed(
    engine: &Engine,
    document: Document,
    policy: &Policy,
    review: impl FnOnce(&[HiddenItem], &mut EditSet),
) -> Result<Audit, EditError> {
    let mut audit = engine
        .analyze(
            document,
            std::slice::from_ref(policy),
            &RequestContext::new(),
        )
        .await
        .expect("analyze succeeds")
        .audit;
    let items = engine
        .hidden_content(&audit, std::slice::from_ref(policy))
        .expect("the preview compiles");
    let mut edits = EditSet::default();
    review(&items, &mut edits);
    edits.apply(&mut audit.report, REVIEWER)?;

    let digest = audit.digest();
    let json = serde_json::to_string(&audit).expect("audit serializes");
    let mut de = serde_json::Deserializer::from_str(&json);
    Ok(engine
        .deserialize_audit(&digest, &mut de)
        .expect("audit deserializes"))
}

/// `audit` serialized and read back, as a host stores one.
fn copy(engine: &Engine, audit: &Audit) -> Audit {
    let json = serde_json::to_string(audit).expect("audit serializes");
    let mut de = serde_json::Deserializer::from_str(&json);
    engine
        .deserialize_audit(&audit.digest(), &mut de)
        .expect("audit deserializes")
}

/// Anonymize `document` under `audit`, returning the output bytes.
async fn anonymize(
    engine: &Engine,
    document: Document,
    policy: &Policy,
    audit: &mut Audit,
) -> Vec<u8> {
    engine
        .anonymize(document, std::slice::from_ref(policy), audit, None)
        .await
        .expect("anonymize succeeds")
        .bytes
        .to_vec()
}

/// The page, reviewed by `review` under `policy`, anonymized.
async fn page_after(policy: &Policy, review: impl FnOnce(&[HiddenItem], &mut EditSet)) -> String {
    let engine = engine();
    let mut audit = reviewed(&engine, page(), policy, review)
        .await
        .expect("the review applies");
    let out = anonymize(&engine, page(), policy, &mut audit).await;
    String::from_utf8(out).expect("html output is utf-8")
}

/// The docx, reviewed by `review` under `policy`, anonymized: its
/// output core properties.
async fn core_after(policy: &Policy, review: impl FnOnce(&[HiddenItem], &mut EditSet)) -> String {
    let engine = engine();
    let mut audit = reviewed(&engine, docx(), policy, review)
        .await
        .expect("the review applies");
    let out = anonymize(&engine, docx(), policy, &mut audit).await;
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(out)).expect("output is a zip package");
    let mut core = String::new();
    archive
        .by_name("docProps/core.xml")
        .expect("core properties survive")
        .read_to_string(&mut core)
        .expect("core properties are utf-8");
    core
}

/// The item whose label is `label`.
fn labelled<'a>(items: &'a [HiddenItem], label: &str) -> &'a HiddenItem {
    items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("no `{label}` item in {items:?}"))
}

/// The page's collaboration item: its comment.
fn comment(items: &[HiddenItem]) -> &HiddenItem {
    items
        .iter()
        .find(|item| item.content == elide::redaction::HiddenContentKind::Collaboration)
        .unwrap_or_else(|| panic!("no comment in {items:?}"))
}

#[tokio::test]
async fn the_preview_marks_each_item_by_label_or_value() {
    let engine = engine();
    let policy = catalog(&["email_address"]);
    let audit = reviewed(&engine, page(), &policy, |_, _| {})
        .await
        .expect("no review");
    let items = engine
        .hidden_content(&audit, std::slice::from_ref(&policy))
        .expect("the preview compiles");

    assert_eq!(items.len(), 3, "author, description, comment: {items:?}");
    assert_eq!(
        labelled(&items, "person_name").removal,
        None,
        "the catalog does not declare person_name, so the author stays",
    );
    assert_eq!(
        labelled(&items, "communications_content").removal,
        Some(Removal::Value),
        "the description holds an address the catalog declares",
    );
    assert_eq!(
        comment(&items).removal,
        None,
        "the comment holds nothing found"
    );

    let policy = catalog(&["email_address", "person_name"]);
    let items = engine
        .hidden_content(&audit, std::slice::from_ref(&policy))
        .expect("the preview compiles");
    assert_eq!(
        labelled(&items, "person_name").removal,
        Some(Removal::Label),
        "declared, the author's label makes it sensitive",
    );
}

#[tokio::test]
async fn anonymize_removes_the_sensitive_items_and_keeps_the_rest() {
    let html = page_after(&catalog(&["email_address"]), |_, _| {}).await;

    assert!(!html.contains("ada@example.com"), "removed; got: {html}");
    assert!(html.contains("Ada Lovelace"), "kept; got: {html}");
    assert!(html.contains("reviewer note"), "kept; got: {html}");
}

#[tokio::test]
async fn a_reviewer_removes_one_item_and_keeps_another() {
    let html = page_after(&catalog(&["email_address"]), |items, edits| {
        edits
            .hidden
            .remove
            .push(HiddenChoice::new(comment(items).id).because("internal note"));
        edits.hidden.keep.push(HiddenChoice::new(
            labelled(items, "communications_content").id,
        ));
    })
    .await;

    assert!(!html.contains("reviewer note"), "removed; got: {html}");
    assert!(
        html.contains("write to ada@example.com"),
        "kept; got: {html}"
    );
    assert!(html.contains("Ada Lovelace"), "untouched; got: {html}");
}

#[tokio::test]
async fn remove_all_removes_every_item() {
    let html = page_after(&catalog(&["email_address"]), |_, edits| {
        edits.hidden.remove_all = Some(RemoveAll::because("publishing externally"));
    })
    .await;

    assert!(!html.contains("Ada Lovelace"), "removed; got: {html}");
    assert!(!html.contains("ada@example.com"), "removed; got: {html}");
    assert!(!html.contains("reviewer note"), "removed; got: {html}");
    assert!(html.contains("Hello"), "the body stays; got: {html}");
}

#[tokio::test]
async fn office_properties_follow_the_same_plan() {
    // The editor is a person, which this catalog declares; the dates
    // and the template are not sensitive and stay.
    let core = core_after(&catalog(&["person_name"]), |items, _| {
        let editor = items
            .iter()
            .find(|item| item.field.as_deref() == Some("lastModifiedBy"))
            .expect("the editor is listed");
        assert_eq!(editor.removal, Some(Removal::Label));
    })
    .await;
    assert!(!core.contains(EDITOR), "removed; got: {core}");
    assert!(core.contains(CREATED), "kept; got: {core}");

    let core = core_after(&catalog(&["email_address"]), |_, _| {}).await;
    assert!(core.contains(EDITOR), "not declared, kept; got: {core}");

    let core = core_after(&catalog(&["email_address"]), |_, edits| {
        edits.hidden.remove_all = Some(RemoveAll::default());
    })
    .await;
    assert!(!core.contains(EDITOR), "removed; got: {core}");
    assert!(!core.contains(CREATED), "removed; got: {core}");
}

#[tokio::test]
async fn contradictory_or_misdirected_decisions_are_rejected() {
    let engine = engine();
    let policy = catalog(&["email_address"]);

    let err = reviewed(&engine, page(), &policy, |items, edits| {
        edits.hidden.remove_all = Some(RemoveAll::default());
        edits.hidden.keep.push(HiddenChoice::new(comment(items).id));
    })
    .await
    .err()
    .expect("remove-all and keep answer the same item differently");
    assert!(
        matches!(err, EditError::Contradiction { .. }),
        "got {err:?}"
    );

    let err = reviewed(&engine, page(), &policy, |items, edits| {
        let id = comment(items).id;
        edits.hidden.remove.push(HiddenChoice::new(id));
        edits.hidden.keep.push(HiddenChoice::new(id));
    })
    .await
    .err()
    .expect("remove and keep answer the same item differently");
    assert!(
        matches!(err, EditError::Contradiction { .. }),
        "got {err:?}"
    );

    let stale = uuid::Uuid::now_v7();
    let err = reviewed(&engine, page(), &policy, |_, edits| {
        edits.hidden.remove.push(HiddenChoice::new(stale));
    })
    .await
    .err()
    .expect("the id names no hidden item");
    assert_eq!(err, EditError::NotHiddenContent { entity_id: stale });
}

#[tokio::test]
async fn a_removed_item_goes_whole_whatever_the_policy_does_to_its_finding() {
    use elide_schema::redaction::{ModalityRedactions, TextRedaction};

    let policy = Policy {
        fallback: Some(ModalityRedactions::text(TextRedaction::Replace {
            template: "[EMAIL]".into(),
        })),
        ..catalog(&["email_address"])
    };
    let html = page_after(&policy, |_, _| {}).await;

    assert!(
        !html.contains("write to"),
        "the item goes whole; got: {html}"
    );
    assert!(!html.contains("ada@example.com"), "removed; got: {html}");
}

#[tokio::test]
async fn the_latest_review_decides_an_item() {
    let engine = engine();
    let policy = catalog(&["email_address"]);
    let decide = |audit: &mut Audit, edits: EditSet| {
        edits
            .apply(&mut audit.report, REVIEWER)
            .expect("the review applies");
    };
    let mut audit = engine
        .analyze(
            page(),
            std::slice::from_ref(&policy),
            &RequestContext::new(),
        )
        .await
        .expect("analyze succeeds")
        .audit;
    let items = engine
        .hidden_content(&audit, std::slice::from_ref(&policy))
        .expect("the preview compiles");
    let description = labelled(&items, "communications_content").id;

    // A first review keeps the description, a second removes it.
    let mut keep = EditSet::default();
    keep.hidden.keep.push(HiddenChoice::new(description));
    decide(&mut audit, keep);
    let mut remove = EditSet::default();
    remove.hidden.remove.push(HiddenChoice::new(description));
    decide(&mut audit, remove);
    let html =
        String::from_utf8(anonymize(&engine, page(), &policy, &mut copy(&engine, &audit)).await)
            .expect("html output is utf-8");
    assert!(
        !html.contains("write to"),
        "the later removal wins; got: {html}"
    );

    // A third keeps it again, over the removal.
    let mut keep = EditSet::default();
    keep.hidden.keep.push(HiddenChoice::new(description));
    decide(&mut audit, keep);
    let items = engine
        .hidden_content(&audit, std::slice::from_ref(&policy))
        .expect("the preview compiles");
    assert!(labelled(&items, "communications_content").kept_by_reviewer);
    let html = String::from_utf8(anonymize(&engine, page(), &policy, &mut audit).await)
        .expect("html output is utf-8");
    assert!(
        html.contains("write to ada@example.com"),
        "the later keep wins, and keeps the item whole; got: {html}"
    );
}

#[tokio::test]
async fn an_item_is_decided_through_hidden_alone() {
    let engine = engine();
    let policy = catalog(&["email_address"]);
    let err = reviewed(&engine, page(), &policy, |items, edits| {
        edits.edit(Edit::<Text>::Suppress(Suppress {
            id: comment(items).id,
            reason: None,
        }));
    })
    .await
    .err()
    .expect("a suppress of a hidden item is rejected");
    assert!(
        matches!(err, EditError::HiddenContentItem { .. }),
        "got {err:?}"
    );
}
