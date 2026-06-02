//! Per-modality [`Strategy → Redaction`] converters.
//!
//! Each module produces the codec-side wire type for one modality's
//! [`RedactionStrategy`]. The applicator calls these once per entry
//! to compute the actual replacement bytes (or rendering parameters)
//! the codec needs.
//!
//! Strategies that depend on services not yet wired through to the
//! applicator (`Encrypt`, `DropColumn`, `DropRow`) keep their match
//! arms here so the wiring stays visible. `Encrypt` currently panics
//! via `unimplemented!()` — the strategy is reachable from config
//! but the underlying `CryptoService::encrypt_bytes` helper isn't
//! built yet. `DropColumn` / `DropRow` surface an explicit error to
//! the caller (see #245). They become real conversions when the
//! supporting machinery lands.
//!
//! [`RedactionStrategy`]: nvisy_core::modality::RedactionStrategy

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "image")]
mod image;
mod tabular;
mod text;

use std::fmt::Write as _;

use nvisy_codec::handler::TextOutput;
use nvisy_core::entity::EntityKind;
use nvisy_core::{Error, Result};
use sha2::Digest;

#[cfg(feature = "audio")]
pub(super) use self::audio::to_audio_redaction;
#[cfg(feature = "image")]
pub(super) use self::image::to_image_redaction;
pub(super) use self::tabular::to_tabular_redaction;
pub(super) use self::text::to_text_redaction;

/// Build a [`TextOutput`] from text-shaped strategy parameters used
/// by both Text and Tabular converters. Returns the audit-side
/// human-readable replacement alongside the codec wire type.
pub(super) fn text_output_from(
    method: TextOutputMethod<'_>,
    original: &str,
    entity_kind: EntityKind,
) -> Result<TextOutput> {
    match method {
        TextOutputMethod::Replace { placeholder } => {
            let replacement = if placeholder.is_empty() {
                format!("[{entity_kind}]")
            } else {
                placeholder.replace("{entityType}", &entity_kind.to_string())
            };
            Ok(TextOutput::Replace { replacement })
        }
        TextOutputMethod::Mask { mask_char } => Ok(TextOutput::Replace {
            replacement: mask_char.to_string().repeat(original.chars().count()),
        }),
        TextOutputMethod::Hash => {
            let digest = sha2::Sha256::digest(original.as_bytes());
            let hex = digest.iter().fold(String::with_capacity(64), |mut acc, b| {
                let _ = write!(acc, "{b:02x}");
                acc
            });
            Ok(TextOutput::Replace { replacement: hex })
        }
        TextOutputMethod::Remove => Ok(TextOutput::Remove),
        TextOutputMethod::Encrypt { key_id } => {
            // Wired end-to-end (key_id flows from the strategy
            // through TextOutputMethod into here) but blocked on a
            // bytes-level CryptoService entry point. Touching the
            // fields keeps the dead-code lint honest.
            let _ = (key_id, original);
            unimplemented!("redaction `encrypt` not yet wired — needs CryptoService::encrypt_bytes")
        }
    }
}

/// Discriminated set of text-shaped strategy variants shared by
/// Text and Tabular converters. `Encrypt` carries its key id so
/// the wiring is visible end-to-end; the body is currently
/// `unimplemented!()` (see [`text_output_from`]).
#[derive(Debug)]
pub(super) enum TextOutputMethod<'a> {
    Replace { placeholder: &'a str },
    Mask { mask_char: char },
    Hash,
    Encrypt { key_id: &'a str },
    Remove,
}

/// Construct the standard "strategy not yet wired" error.
pub(super) fn unsupported(strategy_name: &str) -> Error {
    Error::validation(
        format!("redaction strategy `{strategy_name}` is not yet wired through the applicator"),
        "redaction::apply",
    )
}
