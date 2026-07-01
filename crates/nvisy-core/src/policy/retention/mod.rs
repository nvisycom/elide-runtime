//! Data retention policy types.
//!
//! Wire schema only — the three types here ([`Retention`],
//! [`RetentionScope`], [`RetentionPolicy`]) declare what a
//! policy's `retention` list can hold. Cross-policy resolution
//! ("strictest wins") lives in the engine: see
//! `nvisy_engine::retention::resolve_retention`.

mod duration;
mod scope;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::duration::Retention;
pub use self::scope::RetentionScope;

/// A single retention rule: scope + duration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct RetentionPolicy {
    /// What class of data this applies to.
    pub scope: RetentionScope,
    /// How long to retain data.
    pub retention: Retention,
}
