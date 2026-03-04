//! Geospatial reference data for location-based matching.

mod address;
mod coordinates;
mod region;

pub use address::AddressData;
pub use coordinates::GeoCoordinate;
pub use region::{GeoBounds, GeoShape, RegionData};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Geospatial location variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeospatialVariant {
    /// Geographic region (bounds, circle, or polygon).
    Region(RegionData),
    /// Structured postal address.
    Address(AddressData),
}
