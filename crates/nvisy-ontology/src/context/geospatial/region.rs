//! Geospatial region data for location-based detection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::coordinates::GeoCoordinate;
use crate::math::Polygon;

/// A geographic bounding box defined by its south-west and north-east corners.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeoBounds {
    /// South-west corner.
    pub south_west: GeoCoordinate,
    /// North-east corner.
    pub north_east: GeoCoordinate,
}

impl GeoBounds {
    /// Create bounds from corner coordinates.
    pub fn new(south_west: GeoCoordinate, north_east: GeoCoordinate) -> Self {
        Self {
            south_west,
            north_east,
        }
    }

    /// Check whether a coordinate falls within this bounding box.
    pub fn contains(&self, point: &GeoCoordinate) -> bool {
        point.lat >= self.south_west.lat
            && point.lat <= self.north_east.lat
            && point.lng >= self.south_west.lng
            && point.lng <= self.north_east.lng
    }
}

/// Shape of a geospatial region.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum GeoShape {
    /// Axis-aligned bounding rectangle.
    Bounds(GeoBounds),
    /// Circular region defined by centre and radius.
    Circle {
        /// Centre of the circle.
        centre: GeoCoordinate,
        /// Radius in metres.
        radius_m: f64,
    },
    /// Arbitrary polygon defined by vertices.
    Polygon {
        /// Vertices in order (x = lng, y = lat).
        polygon: Polygon,
    },
}

/// Geospatial region reference data for location-based detection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegionData {
    /// The geographic region.
    #[serde(flatten)]
    pub region: GeoShape,
    /// Optional human-readable name for this region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl RegionData {
    /// Create region data from bounds.
    pub fn from_bounds(bounds: GeoBounds) -> Self {
        Self {
            region: GeoShape::Bounds(bounds),
            name: None,
        }
    }

    /// Create a circular region.
    pub fn from_circle(centre: GeoCoordinate, radius_m: f64) -> Self {
        Self {
            region: GeoShape::Circle { centre, radius_m },
            name: None,
        }
    }

    /// Create a polygon region.
    pub fn from_polygon(polygon: Polygon) -> Self {
        Self {
            region: GeoShape::Polygon { polygon },
            name: None,
        }
    }

    /// Set a name on this region.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}
