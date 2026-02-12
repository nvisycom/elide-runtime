//! Convenience re-exports for common nvisy-pipeline types.

pub use crate::action::Action;
pub use crate::provider::{ConnectedInstance, Provider};

pub use crate::actions::detect_regex::{DetectRegexAction, DetectRegexParams};
pub use crate::actions::detect_dictionary::{DetectDictionaryAction, DetectDictionaryParams, DictionaryDef};
pub use crate::actions::detect_tabular::{DetectTabularAction, DetectTabularParams, ColumnRule};
pub use crate::actions::detect_manual::{DetectManualAction, DetectManualParams};
pub use crate::actions::detect_checksum::{DetectChecksumAction, DetectChecksumParams};
pub use crate::actions::classify::{ClassifyAction, ClassificationResult};
pub use crate::actions::evaluate_policy::{EvaluatePolicyAction, EvaluatePolicyParams};
pub use crate::actions::apply_redaction::ApplyRedactionAction;
pub use crate::actions::emit_audit::{EmitAuditAction, EmitAuditParams};
pub use crate::actions::apply_tabular_redaction::{ApplyTabularRedactionAction, ApplyTabularRedactionParams};
pub use crate::actions::apply_audio_redaction::{ApplyAudioRedactionAction, ApplyAudioRedactionParams};

#[cfg(feature = "image-redaction")]
pub use crate::actions::apply_image_redaction::{ApplyImageRedactionAction, ApplyImageRedactionParams};
#[cfg(feature = "pdf-redaction")]
pub use crate::actions::apply_pdf_redaction::{ApplyPdfRedactionAction, ApplyPdfRedactionParams};
