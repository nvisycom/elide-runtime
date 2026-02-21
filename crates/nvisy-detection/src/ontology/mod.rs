mod entity;
mod location;
mod model;
mod result;
mod selector;
mod annotation;

pub use entity::{DetectionMethod, Entity};
pub use location::{AudioLocation, ImageLocation, TabularLocation, TextLocation, VideoLocation};
pub use model::{ModelInfo, ModelKind};
pub use result::DetectionResult;
pub use selector::EntitySelector;
pub use annotation::{Annotation, AnnotationKind, AnnotationLabel, AnnotationScope};
