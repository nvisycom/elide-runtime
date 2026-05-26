//! [`Context`] — per-call configuration for [`Backend::recognize`]
//! and [`Recognizer::recognize`].
//!
//! Packages everything a recognition call carries: an asserted
//! language (skips detection at the engine layer, doubles as a
//! per-call language hint at the backend layer), a
//! candidate-language set (engine-only — restricts language
//! detection), an entity-kind allowlist (zero-shot backends use it
//! as the requested-kinds hint, the engine post-filters against
//! it), and a correlation UUID for tracing.
//!
//! `text` is **not** a field — it's a separate argument to
//! [`Backend::recognize`] and [`Recognizer::recognize`]. Keeps the
//! context cheap to clone and reusable across multiple calls.
//!
//! [`Backend`]: super::Backend
//! [`Backend::recognize`]: super::Backend::recognize
//! [`Recognizer`]: crate::Recognizer
//! [`Recognizer::recognize`]: crate::Recognizer::recognize

use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use uuid::Uuid;

/// Per-call configuration for [`Backend::recognize`] and
/// [`Recognizer::recognize`].
///
/// All fields are advisory — backends are free to ignore any of
/// them when their model doesn't expose the corresponding knob.
/// Packed into a struct so the trait stays stable as new hint
/// kinds are added.
///
/// [`Backend`]: super::Backend
/// [`Backend::recognize`]: super::Backend::recognize
/// [`Recognizer::recognize`]: crate::Recognizer::recognize
#[derive(Debug, Default, Clone)]
pub struct Context {
    /// Caller-asserted language. When `Some` at the engine layer,
    /// language detection is skipped and the asserted value
    /// becomes the sole entry in [`Artifacts::languages`] with
    /// provenance [`LanguageProvenance::Asserted`]. When passed
    /// directly to a backend, it acts as the language hint.
    ///
    /// Multilingual models may ignore it; monolingual models may
    /// validate it against a configured allowlist and return a
    /// validation error when the hint disagrees.
    ///
    /// [`Artifacts::languages`]: crate::Artifacts::languages
    /// [`LanguageProvenance::Asserted`]: nvisy_ontology::primitive::LanguageProvenance::Asserted
    pub language: Option<LanguageTag>,

    /// Restrict language detection to this subset. Engine-only —
    /// backends ignore it. Ignored when [`language`] is `Some`.
    /// The engine forwards this to
    /// [`LanguagePolicy::detector_for`] each call, so the policy
    /// decides what "restricted to this set" means concretely.
    ///
    /// [`language`]: Self::language
    /// [`LanguagePolicy::detector_for`]: crate::language::LanguagePolicy::detector_for
    pub candidate_languages: Option<Vec<LanguageTag>>,

    /// Entity-kind allowlist. Threaded into zero-shot backends as
    /// the requested-kinds hint so the backend materialises labels
    /// for the asked-about kinds. Backends with a fixed label
    /// vector ignore it. The engine layer post-filters against the
    /// same allowlist.
    pub entity_kinds: Option<Vec<EntityKind>>,

    /// Correlation UUID propagated through the tracing span and,
    /// for remote backends, into the request (the Bento backend
    /// places it on the `x-request-id` header). When `None`,
    /// transports that need a request id generate a UUIDv7
    /// themselves so every request is traceable.
    pub correlation_id: Option<Uuid>,
}

impl Context {
    /// Construct an empty context (no language hint, no kind
    /// filter, no correlation id).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style setter for the language hint.
    pub fn with_language(mut self, language: LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Builder-style setter for the candidate-language set.
    pub fn with_candidate_languages(mut self, languages: Vec<LanguageTag>) -> Self {
        self.candidate_languages = Some(languages);
        self
    }

    /// Builder-style setter for the entity-kind allowlist.
    pub fn with_entity_kinds(mut self, kinds: Vec<EntityKind>) -> Self {
        self.entity_kinds = Some(kinds);
        self
    }

    /// Builder-style setter for the correlation id.
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}
