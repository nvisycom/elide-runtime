//! The pattern recognizers elide ships, built once and shared.

use std::sync::{Arc, LazyLock};

use elide::detection::Analyzer;
use elide::modality::TextRecognizable;
use elide::recognition::Recognizer;
use elide::recognition::context::Enhanced;
use elide::recognition::pattern::{PatternRecognizer, PatternRecognizerBuilder};

const MAX_DICTIONARY_TERM_COUNT: usize = 100_000;

/// Aggregate byte budget across every shipped dictionary's terms.
const MAX_DICTIONARY_TERM_BYTES: usize = 8 * 1024 * 1024;

/// Cap on matchers one request may declare, across every policy.
///
/// Each compiles a regex or a term list on the request path, so
/// this bounds what a caller can make the engine do before
/// detection starts.
pub(super) const MAX_CUSTOM_MATCHERS: usize = 64;

/// Cap on terms in one matcher's list.
pub(super) const MAX_CUSTOM_TERMS: usize = 4096;

/// Compiled-size budget for one caller-supplied regex.
///
/// The `regex` crate does not backtrack, so a pathological pattern
/// costs compile time and memory rather than match time. This
/// bounds both.
pub(super) const MAX_CUSTOM_REGEX_BYTES: usize = 256 * 1024;

/// The built-in pattern recognizer, compiled once for the process.
///
/// Building one compiles the whole shipped regex set — around 50 ms
/// — and the result depends on nothing per-request: the same
/// patterns, dictionaries and limits every time. It used to be
/// rebuilt inside [`attach_pattern`], which runs once per modality
/// per request, so a four-modality deployment paid ~200 ms of regex
/// compilation before reading a byte of the document.
///
/// Shared rather than cloned: `PatternRecognizer` owns compiled
/// automata and is not `Clone`, but an `Arc` is itself a
/// `Recognizer`, so every analyzer attaches the same instance.
///
/// The build takes no caller input — the patterns, dictionaries
/// and limits are all compiled-in constants — so a failure means
/// the shipped set is broken, which no deployment can act on and
/// every request would hit. It panics rather than threading an
/// error no caller can handle.
static BUILTIN_PATTERNS: LazyLock<Arc<Enhanced<PatternRecognizer>>> = LazyLock::new(|| {
    let builder = pattern_with_limits(PatternRecognizer::builder())
        .with_builtin_patterns()
        .with_builtin_dictionaries();
    Arc::new(
        builder
            .build_context_enhanced()
            .expect("the shipped builtin patterns compile"),
    )
});

/// Attach the built-in [`PatternRecognizer`] wrapped in the
/// `Enhanced` context layer.
pub(super) fn attach_pattern<M>(analyzer: Analyzer<M>) -> Analyzer<M>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
    Enhanced<PatternRecognizer>: Recognizer<M> + 'static,
{
    analyzer.with_recognizer(Arc::clone(&BUILTIN_PATTERNS))
}

/// Attach the caller's own matchers, compiled fresh for this
/// request.
///
/// The built-in recognizer is shared and never varies; these do,
/// so they are their own recognizer built per call. A request
/// declaring no matchers builds nothing and pays nothing.
///
/// # Errors
///
/// Returns [`Configuration`](ErrorKind::Configuration) when a
/// matcher names a label its policy does not declare, names a
/// shipped built-in, exceeds a limit, or carries a regex that does
/// not compile.
pub(super) fn pattern_with_limits(builder: PatternRecognizerBuilder) -> PatternRecognizerBuilder {
    builder
        .with_term_count_limit(MAX_DICTIONARY_TERM_COUNT)
        .with_term_bytes_limit(MAX_DICTIONARY_TERM_BYTES)
}
