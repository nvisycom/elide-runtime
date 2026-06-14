//! Entity label types and catalog.
//!
//! Four concerns split across this folder:
//!
//! - [`EntityLabel`] — full catalog entry (name + description +
//!   tags). Authored once per label; consumed by selectors and
//!   audit-rendering tooling that need the metadata.
//! - [`EntityLabelRef`] — name-only handle stored on every
//!   detected [`Entity`]. Cheap-clone wrapper around
//!   [`HipStr<'static>`].
//! - [`EntityLabelCatalog`] — name-indexed lookup over a
//!   collection of `EntityLabel`s. The workspace ships a built-in
//!   catalog constructed from [`EntityLabelCatalog::with_builtins`];
//!   consumers can register custom labels alongside or instead of
//!   the built-ins.
//! - [`builtins`] — every built-in `EntityLabel` constant
//!   (`builtins::PERSON_NAME`, `builtins::EMAIL_ADDRESS`, …) plus
//!   the internal `BUILT_INS` slice the catalog walks at
//!   construction time.
//!
//! [`Entity`]: crate::entity::Entity
//! [`HipStr<'static>`]: hipstr::HipStr

pub mod builtins;
mod catalog;
mod entity_label;
mod reference;

pub use self::catalog::EntityLabelCatalog;
pub use self::entity_label::EntityLabel;
pub use self::reference::EntityLabelRef;
