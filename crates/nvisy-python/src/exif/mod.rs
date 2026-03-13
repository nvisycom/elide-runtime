//! EXIF metadata extraction via the Python backend.
//!
//! Provides [`ExifModule`]: a configured handle that calls
//! `nvisy_ai.extract_exif()` through the [`PythonBridge`](crate::bridge::PythonBridge)
//! to extract EXIF metadata from images. Returns raw JSON values:
//! metadata construction is handled by the caller.

mod module;
mod params;

pub use self::module::ExifModule;
pub use self::params::ExifParams;
