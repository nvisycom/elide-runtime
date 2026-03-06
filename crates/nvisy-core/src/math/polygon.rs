//! Polygon type for non-rectangular regions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::BoundingBox;

/// A single vertex in a [`Polygon`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Vertex {
    /// Horizontal coordinate (pixels or normalized).
    pub x: f64,
    /// Vertical coordinate (pixels or normalized).
    pub y: f64,
}

impl Vertex {
    /// Create a new vertex.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
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

impl Polygon {
    /// Create an empty polygon.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a vertex.
    pub fn push(&mut self, vertex: Vertex) {
        self.vertices.push(vertex);
    }

    /// Returns `true` if the polygon has no vertices.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Number of vertices.
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    /// Compute the axis-aligned bounding box that encloses this polygon.
    ///
    /// Returns [`BoundingBox::default()`] if the polygon has no vertices.
    pub fn bounding_box(&self) -> BoundingBox {
        if self.vertices.is_empty() {
            return BoundingBox::default();
        }

        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);

        for v in &self.vertices {
            min_x = min_x.min(v.x);
            min_y = min_y.min(v.y);
            max_x = max_x.max(v.x);
            max_y = max_y.max(v.y);
        }

        BoundingBox {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }
}
