//! Polygon type for non-rectangular regions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single vertex in a [`Polygon`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Vertex {
    /// Horizontal coordinate (pixels or normalized).
    pub x: f64,
    /// Vertical coordinate (pixels or normalized).
    pub y: f64,
}

/// A closed polygon defined by its vertices.
///
/// Used for rotated or non-rectangular regions such as skewed text
/// detected by OCR. Vertices are ordered (typically clockwise from
/// top-left) and coordinates are `f64` to support both pixel and
/// normalized values.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Polygon {
    /// Ordered vertices defining the polygon outline.
    pub vertices: Vec<Vertex>,
}
