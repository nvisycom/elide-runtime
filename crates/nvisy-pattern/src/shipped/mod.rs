//! Built-in [`Regex`] rules and [`Dictionary`]s shipped with this
//! crate.
//!
//! Each accessor parses an asset file embedded via
//! [`include_bytes!`] and returns a fresh [`Regex`] or
//! [`Dictionary`]. Metadata for dictionaries (entity kind, score,
//! context) is split into a JSON sidecar paired with a CSV / TXT
//! term source; regex rules are self-contained JSON.
//!
//! Use [`patterns::all`] and [`dictionaries::all`] to load the
//! complete shipped set, or pick individual accessors.
//!
//! [`Regex`]: crate::recognition::Regex
//! [`Dictionary`]: crate::recognition::Dictionary

pub mod dictionaries;
pub mod patterns;
