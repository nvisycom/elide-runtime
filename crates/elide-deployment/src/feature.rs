//! Deployment knobs that are not a component lineup.
//!
//! A [`Component`](crate::component::Component) says "this backend
//! exists, here is how to reach it". The settings here configure
//! something that is always present instead, so they have no
//! lineup and no per-request selection: a request cannot ask for a
//! different rate limit.
//!
//! Each one exists because elide offers the capability and a
//! deployment had no way to state it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How language detection runs.
///
/// Language detection is always wired — a recognizer that filters
/// by language needs it — so this narrows rather than enables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Lingua {
    /// The languages detection may return, as BCP-47 tags.
    ///
    /// Empty means unrestricted: every language the build's lingua
    /// model knows. Naming candidates both sharpens accuracy — the
    /// detector is not choosing between seventy languages — and
    /// bounds the work per document.
    ///
    /// Tags are parsed at load, so a typo fails the deployment
    /// rather than silently narrowing detection to nothing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

impl Lingua {
    /// Whether this leaves detection unrestricted.
    #[must_use]
    pub fn is_unrestricted(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Check every candidate is a well-formed BCP-47 tag.
    ///
    /// At load rather than at request compile. A typo'd tag is a
    /// configuration fault, and a deployment that accepted one
    /// would start, serve requests, and fail each one that reached
    /// the text analyzer — reporting a problem in the document's
    /// pipeline for something wrong in a file read at startup.
    ///
    /// # Errors
    ///
    /// Returns [`Invalid`](crate::ErrorKind::Invalid) naming the
    /// offending tag.
    pub fn validate(&self) -> crate::Result<()> {
        for tag in &self.candidates {
            elide::primitive::LanguageTag::parse(tag.clone()).map_err(|err| {
                crate::Error::new(
                    crate::ErrorKind::Invalid,
                    format!(
                        "enrichers.lingua.candidates: `{tag}` is not a valid BCP-47 language tag: {err}"
                    ),
                )
            })?;
        }
        Ok(())
    }
}

/// How often a backend may be called.
///
/// Applies per configured component, wrapping its backend the way
/// usage accounting does. A deployment sharing one hosted model
/// across tenants uses this to keep a single caller from spending
/// the whole quota.
///
/// Absent means unlimited: the runtime adds no limit of its own,
/// since the backend's own quota is the one that matters and a
/// second, invisible limit would be harder to diagnose than no
/// limit at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RateLimit {
    /// Calls permitted per `intervalSeconds`.
    pub requests: u32,
    /// The window `requests` is counted over, in seconds.
    ///
    /// Defaults to sixty, so `requests` alone reads as
    /// requests-per-minute.
    #[serde(default = "default_interval")]
    pub interval_seconds: u32,
    /// How many calls may arrive at once before the rate applies.
    ///
    /// Defaults to `requests`, which lets a burst spend the whole
    /// window's budget immediately and then wait — the behaviour a
    /// caller expects when it has not asked for smoothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<u32>,
    /// How many calls may be in flight at once.
    ///
    /// Independent of the rate: a slow backend can be under its
    /// rate limit and still have a hundred requests outstanding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent: Option<usize>,
    /// How many requests a single batched call may carry.
    ///
    /// A recognizer coalescing a whole document's segments produces one
    /// call, which must then finish inside the provider's request
    /// timeout however large it is. Set this and the batch is split
    /// into consecutive calls, each with its own timeout, so a long
    /// document does not depend on one request completing.
    ///
    /// Unset means the backend is called with whatever batch it is
    /// given, which is correct for a provider that imposes no such
    /// bound of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_batch_size: Option<usize>,
}

/// Sixty seconds: `requests` alone means requests per minute.
const fn default_interval() -> u32 {
    60
}

impl RateLimit {
    /// `requests` calls per minute, bursting up to `requests`.
    #[must_use]
    pub const fn per_minute(requests: u32) -> Self {
        Self {
            requests,
            interval_seconds: 60,
            burst: None,
            max_concurrent: None,
            max_batch_size: None,
        }
    }

    /// The burst allowance, defaulting to one window's budget.
    #[must_use]
    pub const fn burst_or_default(&self) -> u32 {
        match self.burst {
            Some(burst) => burst,
            None => self.requests,
        }
    }

    /// Check this describes a limiter that can admit a call.
    ///
    /// The decorators this configures panic on a zero rate,
    /// interval, burst or permit count — each would be a limiter
    /// that never admits anything. Validating here turns a bad
    /// configuration file into a load error naming the field,
    /// rather than a panic from inside a request.
    ///
    /// # Errors
    ///
    /// Returns [`Invalid`](crate::ErrorKind::Invalid) naming the
    /// offending field, prefixed by `component` so an operator can
    /// find it.
    pub fn validate(&self, component: &str) -> crate::Result<()> {
        let bad = |field: &str, value: String| {
            crate::Error::new(
                crate::ErrorKind::Invalid,
                format!(
                    "component `{component}`: rateLimit.{field} is {value}, which describes a limiter that would never admit a call. Remove the rate limit instead of setting it to zero."
                ),
            )
        };

        if self.requests == 0 {
            return Err(bad("requests", "0".to_owned()));
        }
        if self.interval_seconds == 0 {
            return Err(bad("intervalSeconds", "0".to_owned()));
        }
        if self.burst == Some(0) {
            return Err(bad("burst", "0".to_owned()));
        }
        if self.max_concurrent == Some(0) {
            return Err(bad("maxConcurrent", "0".to_owned()));
        }
        // Zero is rejected rather than read as unlimited: a zero-size window
        // could never carry a request, and `BatchChunked::new` panics on it.
        if self.max_batch_size == Some(0) {
            return Err(bad("maxBatchSize", "0".to_owned()));
        }
        Ok(())
    }
}
