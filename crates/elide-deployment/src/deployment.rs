//! [`Deployment`]: the whole of what an operator decides once.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::component::{Availability, Enrichers, Recognizers};

/// Everything a deployment decides before any request arrives.
///
/// Plain serde, so this is both the shape a configuration file
/// deserializes into and the value [`builder`](Self::builder)
/// produces. A host loads one at startup and hands it to
/// `elide-provider` to build the backends it describes.
///
/// Every field defaults, so a deployment states only what it
/// changes: a file configuring one NER backend and nothing else is
/// a complete, valid deployment.
///
/// ## Validated by construction
///
/// The fields are read-only, and every *loader* runs
/// [`validate`](Self::validate): [`from_toml`], [`from_toml_file`],
/// [`from_toml_resolved`], the [`builder`](Self::builder) and
/// [`try_from_parts`] alike. So a `Deployment` from any of those is
/// one the runtime can build — there is no way to assemble a zero
/// rate limit or a duplicated component name through them and have
/// it survive as a value.
///
/// That is why the fields are not `pub`. A struct literal would be
/// another way in, and the only one that skipped the check.
///
/// ## The one exception
///
/// Deserializing with serde *directly* — `toml::from_str`,
/// `serde_json::from_value` — does **not** validate. Serde's error
/// type cannot carry this crate's
/// [`ErrorKind`](crate::ErrorKind), so validating there would
/// report a semantically impossible configuration as a syntax
/// error. A host taking that path must call
/// [`validate`](Self::validate) itself; that is why the method is
/// public. [`Provider::new`] asserts on it rather than letting an
/// invalid deployment reach request compile.
///
/// [`from_toml`]: Self::from_toml
/// [`from_toml_file`]: Self::from_toml_file
/// [`from_toml_resolved`]: Self::from_toml_resolved
/// [`try_from_parts`]: Self::try_from_parts
/// [`Provider::new`]: https://docs.rs/elide-provider/latest/elide_provider/struct.Provider.html#method.new
#[derive(Debug, Clone, Default)]
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    /// Which components find entities.
    recognizers: Recognizers,
    /// Which components produce the context recognizers read.
    enrichers: Enrichers,
    /// Which of the configured components a caller may reach.
    ///
    /// Defaults to all of them. Narrowing here is the operator's
    /// decision and a request cannot widen it.
    availability: Availability,
}

/// The wire shape, deserialized and then validated.
///
/// Separate from [`Deployment`] so the validation cannot be
/// bypassed: `Deployment`'s own `Deserialize` goes through this,
/// and there is no derived one that would populate the fields
/// unchecked.
// `Default` is here for serde's sake: `#[serde(default)]` needs it
// to fill an omitted field.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct DeploymentWire {
    recognizers: Recognizers,
    enrichers: Enrichers,
    availability: Availability,
}

impl<'de> Deserialize<'de> for Deployment {
    /// Deserializes the wire shape.
    ///
    /// Deliberately does *not* validate. Serde's error type cannot
    /// carry this crate's [`ErrorKind`](crate::ErrorKind), so
    /// validating here would collapse `Invalid` into `Malformed`
    /// and leave an operator unable to tell a syntax error from a
    /// rate limit that can never admit a call.
    ///
    /// Every loader validates explicitly straight after parsing,
    /// which is what keeps the invariant — and a host deserializing
    /// through this impl directly gets a deployment it must
    /// [`validate`](Self::validate) itself. That is the one way to
    /// hold an unvalidated `Deployment`, and it is why `validate`
    /// is public.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = DeploymentWire::deserialize(deserializer)?;
        Ok(Self {
            recognizers: wire.recognizers,
            enrichers: wire.enrichers,
            availability: wire.availability,
        })
    }
}

impl Deployment {
    /// An empty deployment: no components, everything available.
    ///
    /// Valid on its own — elide's built-in pattern recognizers and
    /// language detection need no configuration — so this is the
    /// right starting point for a host that only wants those.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A deployment over already-assembled lineups.
    ///
    /// The fallible counterpart to a struct literal, which this
    /// type does not allow: the parts are checked before they
    /// become a `Deployment`, so every value of this type is one
    /// the runtime can build.
    ///
    /// # Errors
    ///
    /// Returns [`Invalid`](crate::ErrorKind::Invalid) for a rate
    /// limit that would never admit a call, or two components in
    /// one lineup sharing a name. See [`validate`](Self::validate).
    pub fn try_from_parts(
        recognizers: Recognizers,
        enrichers: Enrichers,
        availability: Availability,
    ) -> crate::Result<Self> {
        let deployment = Self {
            recognizers,
            enrichers,
            availability,
        };
        deployment.validate()?;
        Ok(deployment)
    }

    /// Which components find entities.
    #[must_use]
    pub fn recognizers(&self) -> &Recognizers {
        &self.recognizers
    }

    /// Which components produce the context recognizers read.
    #[must_use]
    pub fn enrichers(&self) -> &Enrichers {
        &self.enrichers
    }

    /// Which of the configured components a caller may reach.
    #[must_use]
    pub fn availability(&self) -> &Availability {
        &self.availability
    }

    /// Take the lineups apart, for a consumer that builds backends
    /// from them.
    ///
    /// Consuming rather than borrowing: `elide-provider` moves the
    /// lineups into the provider it builds, and cloning them to do
    /// that would copy every configured component.
    #[must_use]
    pub fn into_parts(self) -> (Recognizers, Enrichers, Availability) {
        (self.recognizers, self.enrichers, self.availability)
    }

    /// Build a deployment in code.
    ///
    /// The counterpart to deserializing one. Both produce the same
    /// value, so a host can start from a file and override in code,
    /// or skip files entirely.
    #[must_use]
    pub fn builder() -> crate::DeploymentBuilder {
        crate::DeploymentBuilder::new()
    }

    /// Read a deployment from a TOML document, expanding `${VAR}`
    /// references against the process environment.
    ///
    /// The usual loader. A deployment file names its secrets
    /// instead of holding them, so the file itself can live
    /// wherever the configuration does while the credentials come
    /// from the environment.
    ///
    /// # Errors
    ///
    /// Returns [`Malformed`] if the document is not well-formed
    /// TOML or does not match this schema, [`MissingVariable`] if
    /// it references an environment variable the process does not
    /// have, and [`Invalid`] if an interpolation is malformed.
    ///
    /// [`Malformed`]: crate::ErrorKind::Malformed
    /// [`MissingVariable`]: crate::ErrorKind::MissingVariable
    /// [`Invalid`]: crate::ErrorKind::Invalid
    pub fn from_toml(source: &str) -> crate::Result<Self> {
        // Delegates rather than repeating the parse: the two
        // loaders promise identical error behaviour, and when this
        // one had its own body it silently skipped `validate` —
        // so a `requests = 0` loaded here and panicked from inside
        // a request instead of failing at startup.
        Self::from_toml_resolved(source, |key| std::env::var(key).ok())
    }

    /// Read a deployment from a TOML document, resolving `${VAR}`
    /// through `lookup` instead of the environment.
    ///
    /// For a host whose secrets do not live in environment
    /// variables, and for testing a file without mutating the
    /// process environment.
    ///
    /// # Errors
    ///
    /// As [`from_toml`](Self::from_toml), with
    /// [`MissingVariable`](crate::ErrorKind::MissingVariable)
    /// raised when `lookup` returns `None`.
    pub fn from_toml_resolved(
        source: &str,
        lookup: impl FnMut(&str) -> Option<String>,
    ) -> crate::Result<Self> {
        let expanded = crate::builder::interpolate(source, lookup)?;
        let deployment: Self =
            toml::from_str(&expanded).map_err(|err| parse_error(&err, &expanded))?;
        deployment.validate()?;
        Ok(deployment)
    }

    /// Read a deployment from the TOML file at `path`, expanding
    /// `${VAR}` against the process environment.
    ///
    /// The same as reading the file and calling
    /// [`from_toml`](Self::from_toml), except that a read failure
    /// names the path. A host whose configuration is not a file —
    /// a database row, a secret store, a config map — uses
    /// [`from_toml`](Self::from_toml) directly.
    ///
    /// # Errors
    ///
    /// Returns [`Unreadable`](crate::ErrorKind::Unreadable) if the
    /// file cannot be read, naming the path and the reason, and
    /// otherwise as [`from_toml`](Self::from_toml).
    pub fn from_toml_file(path: impl AsRef<std::path::Path>) -> crate::Result<Self> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path).map_err(|err| {
            crate::Error::new(
                crate::ErrorKind::Unreadable,
                format!("cannot read `{}`: {err}", path.display()),
            )
        })?;
        Self::from_toml(&source)
    }

    /// Check this deployment describes something the runtime can
    /// build.
    ///
    /// Run automatically by the loaders. A deployment assembled in
    /// code can call it to get the same checks, though the builder
    /// makes most of them unreachable.
    ///
    /// What it catches is the configuration that parses but cannot
    /// run: a rate limit that would never admit a call, or two
    /// components in one lineup sharing a name — which an
    /// allowlist could not tell apart, and whose detections would
    /// be attributed to whichever happened to be first.
    ///
    /// # Errors
    ///
    /// Returns [`Invalid`](crate::ErrorKind::Invalid) naming the
    /// component and field at fault.
    pub fn validate(&self) -> crate::Result<()> {
        for component in &self.recognizers.ner {
            component.validate()?;
        }
        for component in &self.recognizers.llm {
            component.validate()?;
        }
        for component in &self.enrichers.ocr {
            component.validate()?;
        }
        for component in &self.enrichers.stt {
            component.validate()?;
        }

        self.enrichers.lingua.validate()?;

        duplicate_names(
            "recognizers.ner",
            self.recognizers.ner.iter().map(|c| c.name.as_str()),
        )?;
        duplicate_names(
            "recognizers.llm",
            self.recognizers.llm.iter().map(|c| c.name.as_str()),
        )?;
        duplicate_names(
            "enrichers.ocr",
            self.enrichers.ocr.iter().map(|c| c.name.as_str()),
        )?;
        duplicate_names(
            "enrichers.stt",
            self.enrichers.stt.iter().map(|c| c.name.as_str()),
        )?;
        Ok(())
    }
}

/// Refuse a lineup with a repeated name.
///
/// A request's allowlist selects by name, so two components sharing
/// one are indistinguishable to a caller — and every detection from
/// either is attributed to the same name on the audit trail.
fn duplicate_names<'a>(lineup: &str, names: impl Iterator<Item = &'a str>) -> crate::Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        if !seen.insert(name) {
            return Err(crate::Error::new(
                crate::ErrorKind::Invalid,
                format!(
                    "{lineup}: two components are both called `{name}`. A request selects components by name, so the duplicate could never be reached and its detections would be attributed to the other."
                ),
            ));
        }
    }
    Ok(())
}

/// Turn a TOML parse failure into an error that carries no source
/// text.
///
/// `toml`'s own `Display` quotes the offending line to show where
/// the fault is. That is the right behaviour for a config file in
/// general and the wrong one here: by the time parsing runs,
/// `${VAR}` references have been expanded, so the quoted line may
/// hold a resolved credential in plaintext — and a startup failure
/// lands in whatever collects logs.
///
/// The message is rebuilt from the parts that cannot contain
/// document text: what went wrong, and which line it is on. An
/// operator still gets somewhere to look; they read the value from
/// their own file rather than from a log.
fn parse_error(err: &toml::de::Error, expanded: &str) -> crate::Error {
    // The span indexes the expanded text, whose line numbering
    // matches the source: interpolation substitutes within a line
    // and never adds or removes one.
    let location = err.span().map(|span| {
        let line = 1 + expanded[..span.start]
            .bytes()
            .filter(|&b| b == b'\n')
            .count();
        format!(" at line {line}")
    });
    crate::Error::new(
        crate::ErrorKind::Malformed,
        format!(
            "{}{}",
            err.message(),
            location.as_deref().unwrap_or_default()
        ),
    )
}
