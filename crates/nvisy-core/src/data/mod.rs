//! Shared domain data types.

mod encoding;
mod entity_category;
mod entity_kind;
mod entity_sensitivity;
mod layout_kind;

pub use encoding::TextEncoding;
pub use entity_category::EntityCategory;
pub use entity_kind::EntityKind;
pub use entity_sensitivity::EntitySensitivity;
pub use layout_kind::LayoutKind;
