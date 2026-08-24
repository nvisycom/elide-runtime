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
//! 'static>`: an [`Arc`] rather than a [`Box`] so the same
//! handle can seed a [`Fake`] fallback as easily as it can
//! attach on its own. Elide ships a blanket
//! `impl<M, T: Operator<M> + ?Sized> Operator<M> for Arc<T>`
//! (elide #160), so the shared handle is itself an operator and
//! drops straight into [`Rule::label`]. That turns the wire →
//! runtime bridge into a `match` returning one type: no
//! per-variant enum, no fan-out of concrete
//! `WithFallback<primary, fallback>` combinations at the source
//! level.
//!
//! [`Rule::label`]: elide::redaction::Rule::label

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use elide::modality::Modality;
use elide::modality::text::TextReplacement;
use elide::redaction::generator::RandomToken;
use elide::redaction::operators::{
    AesEncrypt, Clamp, Erase, Fake, GeneralizeDate, HmacHash, Keep, KeyProvider, Mask,
    Pseudonymize, PseudonymizeKey, Replace, Sha2Hash, Truncate, TryOperator, WithFallback,
};
use elide::redaction::vault::InMemoryVault;
use elide::redaction::{Anonymizer, Operator};
use elide::{Error, ErrorKind, Result};
use elide_governance::redaction::{ClampBucket, TerminalFallback, TextRedaction};
use uuid::Uuid;

use crate::redaction::compile::Target;

/// Engine-side context the text operator compiler reads at
/// build time.
///
/// Two pieces of per-request state, both keyed by
/// [`PolicyDefinition::id`]:
///
/// - **Pseudonym vaults.** [`TextRedaction::Pseudonymize`]
///   resolves through an [`InMemoryVault`] so every mention of
///   the same entity within a request draws the same surrogate.
///   The vault is *per-policy*: policy A pseudonymising `email`
///   and policy B pseudonymising `email` on the same document
///   don't share a namespace: a customer submitting two policies
///   with different reasoning about coreference stays isolated.
///   Materialised lazily on first request per policy.
/// - **Key provider.** [`TextRedaction::HmacHash`] and
///   [`TextRedaction::Encrypt`] resolve their [`KeyProvider`]
///   through [`key_provider`]. When it is absent, an
///   `HmacHash`/`Encrypt` policy fails to compile with a clear
///   error at request time.
///
/// The provider is per-request state assembled once by
/// `build_anonymize_orchestrator`; every operator built during
/// that request shares the same handle so lookups are cheap
/// [`Arc`] clones.
///
/// [`InMemoryVault`]: elide::redaction::vault::InMemoryVault
/// [`PolicyDefinition::id`]: elide_governance::PolicyDefinition::id
/// [`TextRedaction::Encrypt`]: elide_governance::redaction::TextRedaction::Encrypt
/// [`TextRedaction::HmacHash`]: elide_governance::redaction::TextRedaction::HmacHash
/// [`TextRedaction::Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize
/// [`key_provider`]: Self::key_provider
pub(crate) struct TextOperatorContext {
    /// Engine-level key provider used by every policy's
    /// `HmacHash`/`Encrypt` operator. When absent, those
    /// operators fail to compile with a clear error at request
    /// time.
    pub(crate) key_provider: Option<Arc<dyn KeyProvider>>,
    /// Per-policy pseudonym vaults, materialised lazily on first
    /// access. Wrapped in [`RefCell`] because operator build is
    /// single-threaded (per-request compile) and each vault holds
    /// its own thread-safe [`Arc<Mutex<...>>`] for the concurrent
    /// apply phase.
    pseudonym_vaults: RefCell<PseudonymVaults>,
}

/// Per-policy pseudonym vault registry. One vault per policy id,
/// materialised lazily.
type PseudonymVaults = HashMap<Uuid, InMemoryVault<PseudonymizeKey, TextReplacement>>;

impl TextOperatorContext {
    /// Fresh context for one request. The vault map is empty and
    /// grown lazily.
    pub(crate) fn new(key_provider: Option<Arc<dyn KeyProvider>>) -> Self {
        Self {
            key_provider,
            pseudonym_vaults: RefCell::new(HashMap::new()),
        }
    }

    /// Get or create the pseudonym vault for `policy_id`. The
    /// returned [`InMemoryVault`] clones its inner
    /// `Arc<Mutex<HashMap>>`, so multiple operators built for the
    /// same policy share the same underlying vault state.
    fn pseudonym_vault_for(
        &self,
        policy_id: Uuid,
    ) -> InMemoryVault<PseudonymizeKey, TextReplacement> {
        self.pseudonym_vaults
            .borrow_mut()
            .entry(policy_id)
            .or_insert_with(InMemoryVault::new)
            .clone()
    }
}

/// Type-erased shared handle to a modality-`M` operator.
///
/// `Arc<dyn Operator<M>>`: [`Arc`] rather than [`Box`] so the
/// same handle can seed a [`Fake`] fallback and still attach as
/// its own operator. Elide's blanket forward
/// `impl<M, T: Operator<M> + ?Sized> Operator<M> for Arc<T>`
/// (elide #160) makes the shared handle itself an operator, so
/// it drops directly into [`Rule::label`] with no unwrap.
///
/// [`Rule::label`]: elide::redaction::Rule::label
pub(in crate::redaction) type SharedTextOp<M> = Arc<dyn Operator<M> + Send + Sync + 'static>;

/// Compile `spec` into a boxed operator and attach it to `target`.
///
/// Thin wrapper over [`build`] that immediately hands the built
/// operator to [`Target::attach_with`]. Split so callers that
/// want the operator without attaching (future test hooks, etc.)
/// can reach for `build` directly.
pub(in crate::redaction) fn compile_and_attach<M>(
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
    Pseudonymize<InMemoryVault<PseudonymizeKey, TextReplacement>, RandomToken>: Operator<M>,
    AesEncrypt: Operator<M>,
{
    let policy_id = target.policy_id();
    Ok(target.attach_with(build(spec, ctx, policy_id)?))
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
pub(in crate::redaction) fn build<M>(
    spec: &TextRedaction,
    ctx: &TextOperatorContext,
    policy_id: Uuid,
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
    Pseudonymize<InMemoryVault<PseudonymizeKey, TextReplacement>, RandomToken>: Operator<M>,
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
                .ok_or_else(|| missing_infrastructure(policy_id, "hmac_hash", "KeyProvider"))?;
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
        TextRedaction::Pseudonymize => Arc::new(Pseudonymize::new(
            ctx.pseudonym_vault_for(policy_id),
            RandomToken,
        )),
        TextRedaction::Encrypt => {
            let keys = ctx
                .key_provider
                .clone()
                .ok_or_else(|| missing_infrastructure(policy_id, "encrypt", "KeyProvider"))?;
            Arc::new(AesEncrypt::new(keys))
        }
    })
}

/// Box `primary` bare, or box `WithFallback::new(primary, terminal)`
/// when a fallback is set. One inner match over the 4 terminal
/// variants: the concrete `WithFallback<primary, terminal>` never
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

fn missing_infrastructure(
    policy_id: Uuid,
    operator: &'static str,
    infrastructure: &'static str,
) -> Error {
    Error::new(
        ErrorKind::Configuration,
        format!(
            "policy `{policy_id}` uses `{operator}` which requires a {infrastructure}; \
             configure one via `EngineConfig::key`",
        ),
    )
}
