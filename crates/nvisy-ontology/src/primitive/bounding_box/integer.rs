//! Integer pixel-coordinate bounding box for rendering.

use super::BoundingBox;

/// Integer pixel-coordinate bounding box for rendering operations.
///
/// Converted from [`BoundingBox`] by rounding each field to the nearest
/// integer. Use this at the rendering boundary where pixel-exact
/// coordinates are required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IBoundingBox {
    /// Horizontal offset of the top-left corner in pixels.
    pub x: u32,
    /// Vertical offset of the top-left corner in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl From<&BoundingBox> for IBoundingBox {
    fn from(bb: &BoundingBox) -> Self {
        bb.to_pixel()
    }
}

impl From<BoundingBox> for IBoundingBox {
    fn from(bb: BoundingBox) -> Self {
        Self::from(&bb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bounding_box_rounds() {
        let bb = BoundingBox::new(1.4, 2.6, 3.5, 4.4);
        let px = IBoundingBox::from(bb);
        assert_eq!(px.x, 1);
        assert_eq!(px.y, 3);
        assert_eq!(px.width, 4);
        assert_eq!(px.height, 4);
    }
}
