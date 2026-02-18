//! Domain types: entity, location, and detection result.

mod entity;
mod location;
mod model;
mod result;
mod selector;

pub use entity::{DetectionMethod, Entity};
pub use location::{
    AudioLocation, ImageLocation, TabularLocation, TextLocation, VideoLocation,
};
pub use model::{ModelInfo, ModelKind};
pub use result::DetectionResult;
pub use selector::EntitySelector;
