//! Geographic coordinate primitives.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::PointSchema;

/// A geographic coordinate (latitude/longitude).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeoCoordinate {
    /// Latitude in decimal degrees (−90 to 90).
    pub lat: f64,
    /// Longitude in decimal degrees (−180 to 180).
    pub lng: f64,
}

impl GeoCoordinate {
    /// Create a new coordinate.
    pub fn new(lat: f64, lng: f64) -> Self {
        Self { lat, lng }
    }

    /// Convert to a [`PointSchema`] for polygon operations.
    pub fn to_point(self) -> PointSchema {
        PointSchema {
            x: self.lng,
            y: self.lat,
        }
    }
}
