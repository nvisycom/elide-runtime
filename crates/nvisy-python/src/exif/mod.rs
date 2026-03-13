//! EXIF metadata extraction via the Python backend.
//!
//! Provides [`ExifModule`]: a configured handle that calls
//! `nvisy_ai.extract_exif()` through the [`PythonBridge`]
//! to extract EXIF metadata from images. Returns raw JSON values:
//! metadata construction is handled by the caller.
//!
//! [`PythonBridge`]: crate::bridge::PythonBridge

mod module;
mod params;

pub use self::module::ExifModule;
pub use self::params::ExifParams;
