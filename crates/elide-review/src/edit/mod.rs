//! What a reviewer decided, as data.
//!
//! [`Edit`] is one change; [`EditSet`] is every change for one
//! document, a list per modality. Both deserialize from a request
//! body and validate with no engine in sight — applying them is
//! [`crate::apply`], and it is deliberately a separate concern.
//!
//! - `record` defines the four operations and how two of them
//!   compose or contradict.
//! - `set` collects them per modality and rejects the pairs that
//!   answer one question twice.

mod error;
mod record;
mod set;

pub use self::error::EditError;
pub use self::record::{Add, Edit, Retag, Reviewer, Suppress};
pub use self::set::{EditBucket, EditSet};
