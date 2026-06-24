//! Wire shapes for `elide_core::primitive` geometry types:
//! [`PointSchema`], [`BoundingBoxSchema`], [`PolygonSchema`].

use elide_core::primitive::{BoundingBox, Point, Polygon};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wire-shape proxy for [`elide_core::primitive::Point`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "Point")]
pub struct PointSchema {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl From<PointSchema> for Point {
    fn from(s: PointSchema) -> Self {
        Point::new(s.x, s.y)
    }
}

impl From<Point> for PointSchema {
    fn from(p: Point) -> Self {
        Self { x: p.x, y: p.y }
    }
}

/// Wire-shape proxy for [`elide_core::primitive::BoundingBox`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BoundingBox")]
pub struct BoundingBoxSchema {
    /// Minimum corner (top-left).
    pub min: PointSchema,
    /// Maximum corner (bottom-right).
    pub max: PointSchema,
}

impl From<BoundingBoxSchema> for BoundingBox {
    fn from(s: BoundingBoxSchema) -> Self {
        BoundingBox::new(s.min.into(), s.max.into())
    }
}

impl From<BoundingBox> for BoundingBoxSchema {
    fn from(b: BoundingBox) -> Self {
        Self {
            min: b.min.into(),
            max: b.max.into(),
        }
    }
}

/// Wire-shape proxy for [`elide_core::primitive::Polygon`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(rename = "Polygon", transparent)]
pub struct PolygonSchema(pub Vec<PointSchema>);

impl From<PolygonSchema> for Polygon {
    fn from(s: PolygonSchema) -> Self {
        Polygon::new(s.0.into_iter().map(Into::into).collect::<Vec<Point>>())
    }
}

impl From<Polygon> for PolygonSchema {
    fn from(p: Polygon) -> Self {
        Self(p.vertices().iter().copied().map(Into::into).collect())
    }
}
