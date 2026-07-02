//! Geospatial reference data for location-based matching.

mod address;
mod coordinates;
mod region;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::address::AddressData;
pub use self::coordinates::GeoCoordinate;
pub use self::region::{GeoBounds, GeoShape, RegionData};

/// Geospatial location variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum GeospatialVariant {
    /// Geographic region (bounds, circle, or polygon).
    Region(RegionData),
    /// Structured postal address.
    Address(AddressData),
}
