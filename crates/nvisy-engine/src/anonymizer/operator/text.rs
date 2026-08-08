//! Text-modality operator builder, shared with `tabular`.
//!
//! Cells in a tabular are `TextBacked` in elide, so the same
//! concrete operators (`Erase`, `Keep`, `Mask`, `Replace`,
//! `Sha2Hash`, `HmacHash`, `Truncate`, `Clamp`, `GeneralizeDate`,
//! `Fake`, `Pseudonymize`, `AesEncrypt`) implement `Operator<Text>`
//! *and* `Operator<Tabular>`. This module owns the `TextRedaction
//! -> elide operator` bridge once; per-modality entry files feed
//! their [`Target`] here.
//!
//! Every arm returns an `Arc<dyn Operator<M> + Send + Sync +
//! 'static>` — an [`Arc`] rather than a [`Box`] so the same
//! handle can seed a [`Fake`] fallback as easily as it can
//! attach on its own. Elide ships a blanket
//! `impl<M, T: Operator<M> + ?Sized> Operator<M> for Arc<T>`
//! (elide #160), so the shared handle is itself an operator and
//! drops straight into [`Rule::label`]. That turns the wire →
//! runtime bridge into a `match` returning one type — no
//! per-variant enum, no fan-out of concrete
//! `WithFallback<primary, fallback>` combinations at the source
//! level.
//!
//! [`Rule::label`]: elide::redaction::Rule::label

use std::sync::Arc;

use elide::redaction::Anonymizer;
use elide::redaction::generator::RandomToken;
use elide::redaction::operators::{
    AesEncrypt, Clamp, Erase, Fake, GeneralizeDate, HmacHash, Keep, KeyProvider, Mask,
    Pseudonymize, Replace, Sha2Hash, Truncate, TryOperator, WithFallback,
};
use elide::redaction::vault::InMemoryVault;
use elide_core::entity::LabelRef;
use elide_core::modality::Modality;
use elide_core::modality::text::TextReplacement;
use elide_core::operator::Operator;
use elide_core::{Error, ErrorKind, Result};
use nvisy_schema::policy::redaction::{ClampBucket, TerminalFallback, TextRedaction};

use crate::anonymizer::compile::Target;

/// Engine-side context the text operator compiler reads at
/// build time: which stateful operators have infrastructure
/// wired.
///
/// The pseudonym vault is [`InMemoryVault`] rather than a boxed
/// trait object because [`Vault`] is generic (returns
/// `impl Future`) and its method signatures aren't object-safe.
/// Cross-request pseudonym consistency needs a durable backend
/// — see elide #143 — and will require different plumbing (per-
/// vault-id registry, likely) when that lands.
///
/// [`Vault`]: elide::redaction::vault::Vault
#[derive(Clone)]
pub(crate) struct TextOperatorContext {
    /// Optional key provider for `HmacHash` and `Encrypt`. When
    /// absent, those variants fail to compile with a clear
    /// error — a policy that names them cannot run without one.
    pub(crate) key_provider: Option<Arc<dyn KeyProvider>>,
    /// Per-request pseudonym vault. A `Pseudonymize` variant
    /// resolves through this vault; every mention of the same
    /// entity resolves to the same surrogate within the request.
    /// Cloning shares the underlying map (internal
    /// `Arc<Mutex<...>>`).
    pub(crate) pseudonym_vault: InMemoryVault<(LabelRef, String), TextReplacement>,
}

/// Type-erased shared handle to a modality-`M` operator.
///
/// `Arc<dyn Operator<M>>` — [`Arc`] rather than [`Box`] so the
/// same handle can seed a [`Fake`] fallback and still attach as
/// its own operator. Elide's blanket forward
/// `impl<M, T: Operator<M> + ?Sized> Operator<M> for Arc<T>`
/// (elide #160) makes the shared handle itself an operator, so
/// it drops directly into [`Rule::label`] with no unwrap.
///
/// [`Rule::label`]: elide::redaction::Rule::label
pub(in crate::anonymizer) type SharedTextOp<M> = Arc<dyn Operator<M> + Send + Sync + 'static>;

/// Compile `spec` into a boxed operator and attach it to `target`.
///
/// Thin wrapper over [`build`] that immediately hands the built
/// operator to [`Target::attach_with`]. Split so callers that
/// want the operator without attaching (future test hooks, etc.)
/// can reach for `build` directly.
pub(in crate::anonymizer) fn compile_and_attach<M>(
    spec: &TextRedaction,
    ctx: &TextOperatorContext,
    target: Target<'_, M>,
) -> Result<Anonymizer<M>>
where
    M: Modality + Send + Sync + 'static,
    Erase: Operator<M>,
    Keep: Operator<M>,
    Mask: Operator<M>,
    Replace: Operator<M>,
    Sha2Hash: Operator<M>,
    HmacHash: Operator<M>,
    Truncate: Operator<M>,
    Clamp: TryOperator<M>,
    GeneralizeDate: TryOperator<M>,
    Fake<Replace>: Operator<M>,
    Pseudonymize<InMemoryVault<(LabelRef, String), TextReplacement>, RandomToken>: Operator<M>,
    AesEncrypt: Operator<M>,
{
    Ok(target.attach_with(build(spec, ctx)?))
}

/// Build the concrete elide operator for `spec` and box it.
///
/// One match arm per wire variant. Each arm constructs the
/// concrete elide operator (or the concrete
/// `WithFallback<primary, fallback>` for a declinable primary
/// with an explicit fallback) and returns it boxed. Consumers
/// treat the result as an opaque `impl Operator<M>`; the
/// concrete type lives only on the stack for the duration of the
/// arm's `Arc::new`.
///
/// The where clause names every concrete elide operator this
/// function might construct. Rust trait aliases don't propagate
/// where-clauses to callers (RFC 1733 is unstable), so the list
/// lives inline on each function that needs it rather than
/// behind a supertrait. Text and tabular both satisfy it.
pub(in crate::anonymizer) fn build<M>(
    spec: &TextRedaction,
    ctx: &TextOperatorContext,
) -> Result<SharedTextOp<M>>
where
    M: Modality + Send + Sync + 'static,
    Erase: Operator<M>,
    Keep: Operator<M>,
    Mask: Operator<M>,
    Replace: Operator<M>,
    Sha2Hash: Operator<M>,
    HmacHash: Operator<M>,
    Truncate: Operator<M>,
    Clamp: TryOperator<M>,
    GeneralizeDate: TryOperator<M>,
    Fake<Replace>: Operator<M>,
    Pseudonymize<InMemoryVault<(LabelRef, String), TextReplacement>, RandomToken>: Operator<M>,
    AesEncrypt: Operator<M>,
{
    Ok(match spec {
        TextRedaction::Erase => Arc::new(Erase),
        TextRedaction::Keep => Arc::new(Keep),
        TextRedaction::Mask {
            mask_char,
            keep_prefix,
            keep_suffix,
        } => Arc::new(build_mask(*mask_char, *keep_prefix, *keep_suffix)),
        TextRedaction::Replace { template } => Arc::new(Replace::new(template.clone())),
        TextRedaction::Hash { algorithm, salt } => {
            let mut op = Sha2Hash::new(*algorithm);
            if let Some(s) = salt {
                op = op.with_salt(s.as_bytes().to_vec());
            }
            Arc::new(op)
        }
        TextRedaction::HmacHash { algorithm } => {
            let keys = ctx
                .key_provider
                .clone()
                .ok_or_else(|| missing_infrastructure("hmac_hash", "KeyProvider"))?;
            Arc::new(HmacHash::new(*algorithm, keys))
        }
        TextRedaction::Truncate {
            keep_prefix,
            keep_suffix,
        } => Arc::new(Truncate::new(*keep_prefix, *keep_suffix)),
        TextRedaction::Clamp {
            ceiling,
            ceiling_bucket,
            floor,
            floor_bucket,
            fallback,
        } => {
            let clamp = build_clamp(
                *ceiling,
                ceiling_bucket.as_ref(),
                *floor,
                floor_bucket.as_ref(),
            )?;
            box_with_optional_fallback(clamp, fallback.as_ref())
        }
        TextRedaction::GeneralizeDate {
            granularity,
            style,
            fallback,
        } => {
            let generalize = GeneralizeDate::new(*granularity).with_style(*style);
            box_with_optional_fallback(generalize, fallback.as_ref())
        }
        TextRedaction::Fake {
            default_language,
            seed,
            fallback_template,
        } => {
            let mut op = Fake::new(Replace::new(fallback_template.clone()));
            if let Some(lang) = default_language {
                op = op.with_default_language(lang.clone());
            }
            if let Some(s) = seed {
                op = op.with_seed(*s);
            }
            Arc::new(op)
        }
        TextRedaction::Pseudonymize => {
            Arc::new(Pseudonymize::new(ctx.pseudonym_vault.clone(), RandomToken))
        }
        TextRedaction::Encrypt => {
            let keys = ctx
                .key_provider
                .clone()
                .ok_or_else(|| missing_infrastructure("encrypt", "KeyProvider"))?;
            Arc::new(AesEncrypt::new(keys))
        }
    })
}

/// Box `primary` bare, or box `WithFallback::new(primary, terminal)`
/// when a fallback is set. One inner match over the 4 terminal
/// variants — the concrete `WithFallback<primary, terminal>` never
/// escapes this function.
fn box_with_optional_fallback<M, P>(
    primary: P,
    fallback: Option<&TerminalFallback>,
) -> SharedTextOp<M>
where
    M: Modality + Send + Sync + 'static,
    P: TryOperator<M> + Send + Sync + 'static,
    Erase: Operator<M>,
    Keep: Operator<M>,
    Replace: Operator<M>,
    Mask: Operator<M>,
{
    match fallback {
        None => Arc::new(primary),
        Some(TerminalFallback::Erase) => Arc::new(WithFallback::new(primary, Erase)),
        Some(TerminalFallback::Keep) => Arc::new(WithFallback::new(primary, Keep)),
        Some(TerminalFallback::Replace { template }) => {
            Arc::new(WithFallback::new(primary, Replace::new(template.clone())))
        }
        Some(TerminalFallback::Mask {
            mask_char,
            keep_prefix,
            keep_suffix,
        }) => Arc::new(WithFallback::new(
            primary,
            build_mask(*mask_char, *keep_prefix, *keep_suffix),
        )),
    }
}

fn build_mask(mask_char: char, keep_prefix: usize, keep_suffix: usize) -> Mask {
    Mask::new(mask_char)
        .with_keep_prefix(keep_prefix)
        .with_keep_suffix(keep_suffix)
}

/// Build an [`elide::Clamp`] from wire fields, validating the
/// bucket/threshold pairing (a threshold without a bucket, or
/// vice versa, is an author error).
fn build_clamp(
    ceiling: Option<f64>,
    ceiling_bucket: Option<&ClampBucket>,
    floor: Option<f64>,
    floor_bucket: Option<&ClampBucket>,
) -> Result<Clamp> {
    validate_pair("ceiling", ceiling.is_some(), ceiling_bucket.is_some())?;
    validate_pair("floor", floor.is_some(), floor_bucket.is_some())?;
    let mut op = Clamp::new();
    if let (Some(threshold), Some(bucket)) = (ceiling, ceiling_bucket) {
        op = bucket.attach_ceiling(op, threshold);
    }
    if let (Some(threshold), Some(bucket)) = (floor, floor_bucket) {
        op = bucket.attach_floor(op, threshold);
    }
    Ok(op)
}

fn validate_pair(side: &'static str, threshold_set: bool, bucket_set: bool) -> Result<()> {
    if threshold_set != bucket_set {
        return Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "policy compile: `clamp.{side}` and `clamp.{side}_bucket` \
                 must be set together",
            ),
        ));
    }
    Ok(())
}

fn missing_infrastructure(operator: &'static str, infrastructure: &'static str) -> Error {
    Error::new(
        ErrorKind::Configuration,
        format!(
            "policy compile: `{operator}` requires an engine-side {infrastructure}; \
             wire one via `Engine::with_key_provider` (or the matching setter)",
        ),
    )
}
