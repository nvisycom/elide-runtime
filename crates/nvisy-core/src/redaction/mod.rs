//! Redaction method tag enums.
//!
//! Lightweight identifiers that name a redaction algorithm without
//! carrying any configuration data.
//!
//! - [`TextRedactionMethod`] — text/tabular strategies (mask, replace, hash, etc.)
//! - [`ImageRedactionMethod`] — image/video strategies (blur, block, pixelate)
//! - [`AudioRedactionMethod`] — audio strategies (silence, remove)
//! - [`RedactionMethod`] — unified wrapper

mod method;

pub use method::{
    AudioRedactionMethod, ImageRedactionMethod, RedactionMethod, TextRedactionMethod,
};
