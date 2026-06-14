//! Shared recognizer + redaction registry constructors and dedup
//! params used by every codec E2E test.

use nvisy_core::entity::builtins;
use nvisy_core::modality::Modality;
use nvisy_core::primitive::ConfidenceThreshold;
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::deduplication::LayerParams;
use nvisy_toolkit::redaction::anonymizer::{Mask, Replace};
use nvisy_toolkit::redaction::{Anonymizer, RedactionRegistry};

/// Build the shipped pattern recognizer from every built-in pattern.
pub fn shipped_recognizer() -> PatternRecognizer {
    PatternRecognizer::builder()
        .with_registry(PatternRegistry::builtin())
        .build()
        .expect("shipped recognizer builds")
}

/// Per-test redaction registry: every entity kind the shipped
/// patterns can emit is mapped to a deterministic operator so test
/// assertions can spot-check the replacement tokens.
///
/// - emails, phones, IBANs, government ids, IPs → `[{label}]`
/// - payment cards → `Mask::stars()` (digits masked, no token)
pub fn redaction_registry<M>() -> RedactionRegistry<M>
where
    M: Modality,
    Replace: Anonymizer<M>,
    Mask: Anonymizer<M>,
{
    RedactionRegistry::<M>::new()
        .insert_label(
            builtins::EMAIL_ADDRESS.label_ref(),
            Replace::new("[{label}]"),
        )
        .insert_label(
            builtins::PHONE_NUMBER.label_ref(),
            Replace::new("[{label}]"),
        )
        .insert_label(builtins::IBAN.label_ref(), Replace::new("[{label}]"))
        .insert_label(
            builtins::GOVERNMENT_ID.label_ref(),
            Replace::new("[{label}]"),
        )
        .insert_label(builtins::IP_ADDRESS.label_ref(), Replace::new("[{label}]"))
        .insert_label(builtins::PAYMENT_CARD.label_ref(), Mask::stars())
}

/// Standard dedup params: a `0.5` confidence threshold drops the
/// low-confidence ISO-639 short-code matches from the languages
/// dictionary (see `assets/dictionaries/general/languages.toml`'s
/// `column_scores`).
pub fn dedup_params() -> LayerParams {
    LayerParams {
        confidence_threshold: Some(ConfidenceThreshold::new(0.5).unwrap()),
        ..LayerParams::default()
    }
}
