//! Text rendering and redaction primitives.
//!
//! Low-level utilities for applying text replacements and cell-level
//! masking, used by pipeline redaction actions.
//!
//! # Sub-modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`replace`] | Byte-offset text replacement engine |
//! | [`mask`] | Cell-level masking and hashing utilities |

pub mod mask;
pub mod replace;

pub use mask::{hash_string, mask_cell};
pub use replace::{apply_replacements, PendingReplacement};
