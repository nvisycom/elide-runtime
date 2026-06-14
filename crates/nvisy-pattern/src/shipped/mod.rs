//! Built-in [`Regex`] rules and [`Dictionary`]s shipped with this
//! crate.
//!
//! Each accessor parses an asset file embedded via
//! [`include_str!`] and returns a fresh [`Regex`] or
//! [`Dictionary`]. Metadata for dictionaries (entity label, score,
//! context) is split into a TOML sidecar paired with a CSV / TXT
//! term source; regex rules are self-contained TOML.
//!
//! Use [`patterns::all`] and [`dictionaries::all`] to load the
//! complete shipped set, or pick individual accessors.
//!
//! [`Regex`]: crate::Regex
//! [`Dictionary`]: crate::Dictionary

pub mod dictionaries;
pub mod patterns;
