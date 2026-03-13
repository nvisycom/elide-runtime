//! [`ExifParams`]: configuration for EXIF extraction calls.

/// Parameters for EXIF extraction.
#[derive(Debug, Clone, Copy)]
pub struct ExifParams {
    /// Whether to include GPS coordinates in the output.
    pub include_gps: bool,
    /// Whether to include thumbnail data in the output.
    pub include_thumbnail: bool,
}

impl Default for ExifParams {
    fn default() -> Self {
        Self {
            include_gps: true,
            include_thumbnail: false,
        }
    }
}
