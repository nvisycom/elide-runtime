//! Authored recognition plan and caller-supplied annotations.
//!
//! Serialisable description of how to build an analyzer for a
//! request. Symmetric with [`policy`]. Where policy describes
//! redaction governance (which entities to hide and how), the
//! plan describes recognition (which entities to find and how).
//! Both are pure data; the engine compiles them into elide
//! runtime values at request time.
//!
//! [`policy`]: crate::policy

pub use nvisy_schema::annotation::{Annotations, Exclusion, Inclusion};
pub use nvisy_schema::plan::{
    AnalyzerParams, AnyAnnotations, CustomDictionary, CustomDictionaryTerm, CustomPatternContext,
    CustomPatternRule, CustomPatternVariant, DeduplicationParams, EnricherParams,
    LanguageEnricherParams, MAX_REGEX_SOURCE_LEN, MergingStrategyParams, OcrBackendParams,
    OcrEnricherParams, PatternRecognizerParams, ProviderSelection, RecognizerParams, ScopeParams,
    SttBackendParams, SttEnricherParams, TiebreakerParams,
};
