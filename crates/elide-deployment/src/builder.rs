//! Building a [`Deployment`] in code, and expanding `${VAR}`
//! references in a configuration document.

use hipstr::HipStr;

use crate::component::{
    Availability, Component, Enrichers, LlmBackend, NerBackend, OcrBackend, Recognizers, SttBackend,
};
use crate::feature::{Lingua, RateLimit};
use crate::{Deployment, Error, ErrorKind, Result};

/// Expand `${VAR}` references in `source`, resolving each through
/// `lookup`.
///
/// Textual, deliberately: it runs before parsing, so a variable can
/// hold any scalar a configuration document accepts rather than
/// only a string field's contents. The expansion is not recursive —
/// a value containing `${...}` is left alone — so a credential that
/// happens to look like a reference cannot cause a second lookup.
///
/// Running before the parse has one consequence worth knowing: the
/// whole document is text at that point, comments included, so a
/// reference written in a comment is expanded like any other and
/// fails the load if it is unset. Escape it as `$${VAR}` to write
/// one for illustration.
///
/// A missing variable is an error, never an empty string. A
/// credential that silently became `""` would be sent to the
/// backend as a credential, and the failure would surface as an
/// authentication error far from its cause.
///
/// `$${VAR}` is an escaped literal `${VAR}`, for the rare document
/// that needs the text itself.
///
/// # Errors
///
/// Returns [`MissingVariable`](ErrorKind::MissingVariable) for a
/// reference `lookup` cannot resolve, and
/// [`Invalid`](ErrorKind::Invalid) for an unterminated `${`.
pub(crate) fn interpolate(
    source: &str,
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> Result<String> {
    // The common case is a document with no references at all, and
    // it should not pay for a rebuild.
    if !source.contains("${") {
        return Ok(source.to_owned());
    }

    let mut out = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(at) = rest.find("${") {
        // `$${VAR}` escapes: emit the literal `${VAR}` and resume
        // after it, so the inner text is never looked up.
        //
        // Indexing `at - 1` by byte is safe: `$` is ASCII, and every
        // UTF-8 continuation byte is >= 0x80, so a byte equal to
        // `b'$'` is always a whole character. No multi-byte
        // character ends in 0x24.
        if at > 0 && rest.as_bytes()[at - 1] == b'$' {
            out.push_str(&rest[..at - 1]);
            // The search is bounded to this token, not the rest of
            // the document. Unbounded, a `$${` with no closing brace
            // of its own would consume up to the *next* reference's
            // `}` and emit that reference verbatim — so a real
            // `${VAR}` would silently become the literal seven
            // characters instead of being looked up or reported.
            let inner = &rest[at + 2..];
            let Some(end) = inner.find('}') else {
                return Err(unterminated(&rest[at..]));
            };
            // A `${` inside the braces means the token never closed:
            // the `}` found belongs to something further on.
            if inner[..end].contains("${") {
                return Err(unterminated(&rest[at..]));
            }
            out.push_str(&rest[at..=at + 2 + end]);
            rest = &inner[end + 1..];
            continue;
        }

        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let Some(end) = after.find('}') else {
            return Err(unterminated(&rest[at..]));
        };
        let key = &after[..end];

        if key.is_empty() {
            return Err(Error::new(
                ErrorKind::Invalid,
                "empty interpolation `${}`: name the variable to read, or escape the \
text as `$${}`",
            ));
        }

        let value = lookup(key).ok_or_else(|| {
            Error::new(
                ErrorKind::MissingVariable,
                format!(
                    "`${{{key}}}` is referenced but `{key}` is not set. Set it, or \
remove the reference — an unset variable is never treated as \
empty, since an empty credential would reach the backend as a \
credential."
                ),
            )
        })?;
        out.push_str(&value);
        rest = &after[end + 1..];
    }

    out.push_str(rest);
    Ok(out)
}

/// An unterminated `${` at `at`, quoted back far enough to find.
fn unterminated(at: &str) -> Error {
    let shown: String = at.chars().take(24).collect();
    Error::new(
        ErrorKind::Invalid,
        format!("unterminated interpolation starting at `{shown}`: expected a closing `}}`"),
    )
}

/// Builds a [`Deployment`] in code.
///
/// Lowers into exactly the value a configuration file deserializes
/// into, so the two paths cannot drift. Lineups a deployment does
/// not configure are simply never mentioned.
///
/// Each `ner`/`llm`/`ocr`/`stt` call opens a [`ComponentEntry`],
/// which takes that component's optional settings and returns here
/// on [`finish`](ComponentEntry::finish).
#[derive(Debug, Clone, Default)]
pub struct DeploymentBuilder {
    recognizers: Recognizers,
    enrichers: Enrichers,
    availability: Availability,
}

impl DeploymentBuilder {
    /// An empty builder: no components, everything available.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a NER recognizer called `name`.
    #[must_use]
    pub fn ner(
        self,
        name: impl Into<HipStr<'static>>,
        backend: NerBackend,
    ) -> ComponentEntry<NerBackend> {
        ComponentEntry::new(self, name, backend)
    }

    /// Add an LLM recognizer called `name`.
    #[must_use]
    pub fn llm(
        self,
        name: impl Into<HipStr<'static>>,
        backend: LlmBackend,
    ) -> ComponentEntry<LlmBackend> {
        ComponentEntry::new(self, name, backend)
    }

    /// Add an OCR enricher called `name`.
    #[must_use]
    pub fn ocr(
        self,
        name: impl Into<HipStr<'static>>,
        backend: OcrBackend,
    ) -> ComponentEntry<OcrBackend> {
        ComponentEntry::new(self, name, backend)
    }

    /// Add an STT enricher called `name`.
    #[must_use]
    pub fn stt(
        self,
        name: impl Into<HipStr<'static>>,
        backend: SttBackend,
    ) -> ComponentEntry<SttBackend> {
        ComponentEntry::new(self, name, backend)
    }

    /// Restrict which configured components a caller may reach.
    #[must_use]
    pub fn availability(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }

    /// Narrow language detection to `candidates`, as BCP-47 tags.
    #[must_use]
    pub fn languages<I, S>(mut self, candidates: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.enrichers.lingua = Lingua {
            candidates: candidates.into_iter().map(Into::into).collect(),
        };
        self
    }

    /// The deployment this builder describes.
    ///
    /// Fallible because a `Deployment` is validated by
    /// construction: a rate limit that would never admit a call, or
    /// two components in one lineup sharing a name, is refused here
    /// rather than at first use. Most builder chains cannot produce
    /// either, but a caller passing a [`RateLimit`] it computed can.
    ///
    /// # Errors
    ///
    /// As [`Deployment::try_from_parts`].
    pub fn build(self) -> Result<Deployment> {
        Deployment::try_from_parts(self.recognizers, self.enrichers, self.availability)
    }
}

/// One component mid-construction: its name and backend are set,
/// its optional settings are not.
///
/// Returned by [`DeploymentBuilder::ner`] and its siblings.
/// [`finish`](Self::finish) appends it to its lineup and hands the
/// builder back.
#[derive(Debug, Clone)]
pub struct ComponentEntry<B> {
    builder: DeploymentBuilder,
    component: Component<B>,
}

impl<B> ComponentEntry<B> {
    /// Start a component called `name` running `backend`.
    ///
    /// Which lineup it joins is not recorded: the backend type
    /// decides it, since `finish` is implemented once per backend
    /// and each impl appends to one lineup.
    fn new(builder: DeploymentBuilder, name: impl Into<HipStr<'static>>, backend: B) -> Self {
        Self {
            builder,
            component: Component::new(name, backend),
        }
    }

    /// Limit how often this component's backend may be called.
    ///
    /// Delegates to [`Component::with_rate_limit`] rather than
    /// setting the field, so the two authoring paths cannot drift.
    #[must_use]
    pub fn rate_limit(mut self, rate_limit: RateLimit) -> Self {
        self.component = self.component.with_rate_limit(rate_limit);
        self
    }

    /// Place this component in the given deployment-defined
    /// families, which an [`Availability`] or a request's selection
    /// can then name instead of listing components one by one.
    ///
    /// Delegates to [`Component::with_tags`], as
    /// [`rate_limit`](Self::rate_limit) does.
    ///
    /// [`Availability`]: crate::component::Availability
    #[must_use]
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<HipStr<'static>>,
    {
        self.component = self.component.with_tags(tags);
        self
    }
}

/// `finish` is written once per backend rather than generically:
/// each appends to a differently-typed `Vec`, and the backend type
/// is what fixes the lineup a component joins.
macro_rules! finish_for {
    ($backend:path, $field:ident, $holder:ident) => {
        impl ComponentEntry<$backend> {
            /// Append this component to its lineup and return to the
            /// deployment builder.
            #[must_use]
            pub fn finish(self) -> DeploymentBuilder {
                let Self {
                    mut builder,
                    component,
                } = self;
                builder.$holder.$field.push(component);
                builder
            }
        }
    };
}

finish_for!(NerBackend, ner, recognizers);
finish_for!(LlmBackend, llm, recognizers);
finish_for!(OcrBackend, ocr, enrichers);
finish_for!(SttBackend, stt, enrichers);
