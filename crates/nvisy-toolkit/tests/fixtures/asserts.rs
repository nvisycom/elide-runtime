//! Entity-presence and redaction-output assertion helpers for the
//! codec E2E tests.

use nvisy_core::entity::{Entity, EntityLabelRef, builtins};
use nvisy_core::modality::{Tabular, Text};

/// Assert at least one `Entity<Text>` of `kind` matches `needle`
/// when its location is sliced against `source`.
#[track_caller]
pub fn assert_text_entity(
    source: &str,
    entities: &[Entity<Text>],
    label: EntityLabelRef,
    needle: &str,
) {
    let hit = entities
        .iter()
        .any(|e| e.label == label && &source[e.location.start..e.location.end] == needle);
    assert!(
        hit,
        "expected `{needle}` as {label:?}; got: {:?}",
        entities
            .iter()
            .map(|e| (e.label.clone(), &source[e.location.start..e.location.end]))
            .collect::<Vec<_>>()
    );
}

/// Assert at least one `Entity<Tabular>` of `kind` lives in cell
/// `(row, col)` and its intra-cell range slices to `needle`.
#[track_caller]
pub fn assert_tabular_entity(
    cell_value: &str,
    entities: &[Entity<Tabular>],
    label: EntityLabelRef,
    row: u32,
    col: u32,
    needle: &str,
) {
    let hit = entities.iter().any(|e| {
        if e.label != label {
            return false;
        }
        if e.location.row_index != row || e.location.column_index != col {
            return false;
        }
        let start = e.location.start_offset.unwrap_or(0);
        let end = e.location.end_offset.unwrap_or(cell_value.len());
        cell_value.get(start..end) == Some(needle)
    });
    assert!(
        hit,
        "expected `{needle}` as {label:?} at ({row},{col}); got: {:?}",
        entities
            .iter()
            .map(|e| (
                e.label.clone(),
                e.location.row_index,
                e.location.column_index
            ))
            .collect::<Vec<_>>()
    );
}

/// Assert every `needle` in `pii` is absent from `redacted` —
/// detection-and-redact removed each sensitive substring end-to-end.
#[track_caller]
pub fn assert_pii_removed(redacted: &str, pii: &[&str]) {
    for needle in pii {
        assert!(
            !redacted.contains(needle),
            "PII `{needle}` survived redaction: {redacted}"
        );
    }
}

/// Assert every replacement `token` is present in `redacted` —
/// the operator fired and wrote its substitution.
#[track_caller]
pub fn assert_tokens_present(redacted: &str, tokens: &[&str]) {
    for token in tokens {
        assert!(
            redacted.contains(token),
            "replacement token `{token}` missing from output: {redacted}"
        );
    }
}
