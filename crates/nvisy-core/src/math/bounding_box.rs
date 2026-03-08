//! Axis-aligned bounding box types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Axis-aligned bounding box for image-based entity locations.
///
/// Coordinates are `f64` to support both pixel and normalized (0.0–1.0)
/// values from detection models. Use [`BoundingBoxU32`] (or [`Into`])
/// when integer pixel coordinates are needed for rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BoundingBox {
    /// Horizontal offset of the top-left corner (pixels or normalized).
    pub x: f64,
    /// Vertical offset of the top-left corner (pixels or normalized).
    pub y: f64,
    /// Width of the bounding box.
    pub width: f64,
    /// Height of the bounding box.
    pub height: f64,
}

impl BoundingBox {
    /// Create a new bounding box.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Area of the bounding box.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Right edge (`x + width`).
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// Bottom edge (`y + height`).
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Center point `(cx, cy)`.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Returns `true` if the point `(px, py)` lies inside the box.
    pub fn contains_point(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    /// Returns `true` if this box overlaps with `other`.
    pub fn overlaps(&self, other: &BoundingBox) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Returns the intersection of two boxes, or `None` if they don't overlap.
    pub fn intersection(&self, other: &BoundingBox) -> Option<BoundingBox> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if x < right && y < bottom {
            Some(BoundingBox::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Returns the smallest box that encloses both `self` and `other`.
    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        BoundingBox::new(x, y, right - x, bottom - y)
    }

    /// Intersection-over-union (IoU) with `other`.
    ///
    /// Returns 0.0 if the boxes don't overlap or if both have zero area.
    pub fn iou(&self, other: &BoundingBox) -> f64 {
        let inter = match self.intersection(other) {
            Some(b) => b.area(),
            None => return 0.0,
        };
        let union = self.area() + other.area() - inter;
        if union == 0.0 { 0.0 } else { inter / union }
    }

    /// Returns the smallest box enclosing all boxes in the iterator.
    ///
    /// Returns [`BoundingBox::default()`] if the iterator is empty.
    pub fn enclosing<'a>(mut iter: impl Iterator<Item = &'a BoundingBox>) -> BoundingBox {
        match iter.next() {
            None => BoundingBox::default(),
            Some(first) => iter.fold(*first, |acc, b| acc.union(b)),
        }
    }

    /// Convert to integer pixel coordinates by rounding each field.
    pub fn to_u32(&self) -> BoundingBoxU32 {
        BoundingBoxU32 {
            x: self.x.round() as u32,
            y: self.y.round() as u32,
            width: self.width.round() as u32,
            height: self.height.round() as u32,
        }
    }
}

/// Integer pixel-coordinate bounding box for rendering operations.
///
/// Converted from [`BoundingBox`] by rounding each field to the nearest
/// integer. Use this at the rendering boundary where pixel-exact
/// coordinates are required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoundingBoxU32 {
    /// Horizontal offset of the top-left corner in pixels.
    pub x: u32,
    /// Vertical offset of the top-left corner in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl From<&BoundingBox> for BoundingBoxU32 {
    fn from(bb: &BoundingBox) -> Self {
        bb.to_u32()
    }
}

impl From<BoundingBox> for BoundingBoxU32 {
    fn from(bb: BoundingBox) -> Self {
        Self::from(&bb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area() {
        let bb = BoundingBox::new(0.0, 0.0, 10.0, 5.0);
        assert!((bb.area() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn edges_and_center() {
        let bb = BoundingBox::new(10.0, 20.0, 30.0, 40.0);
        assert!((bb.right() - 40.0).abs() < f64::EPSILON);
        assert!((bb.bottom() - 60.0).abs() < f64::EPSILON);
        let (cx, cy) = bb.center();
        assert!((cx - 25.0).abs() < f64::EPSILON);
        assert!((cy - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn contains_point() {
        let bb = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        assert!(bb.contains_point(5.0, 5.0));
        assert!(bb.contains_point(0.0, 0.0));
        assert!(bb.contains_point(10.0, 10.0));
        assert!(!bb.contains_point(11.0, 5.0));
    }

    #[test]
    fn overlaps() {
        let a = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BoundingBox::new(5.0, 5.0, 10.0, 10.0);
        let c = BoundingBox::new(10.0, 0.0, 10.0, 10.0);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c)); // touching at edge = no overlap
    }

    #[test]
    fn intersection() {
        let a = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BoundingBox::new(5.0, 5.0, 10.0, 10.0);
        let i = a.intersection(&b).unwrap();
        assert!((i.x - 5.0).abs() < f64::EPSILON);
        assert!((i.y - 5.0).abs() < f64::EPSILON);
        assert!((i.width - 5.0).abs() < f64::EPSILON);
        assert!((i.height - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn union_boxes() {
        let a = BoundingBox::new(0.0, 0.0, 5.0, 5.0);
        let b = BoundingBox::new(3.0, 3.0, 7.0, 7.0);
        let u = a.union(&b);
        assert!((u.x).abs() < f64::EPSILON);
        assert!((u.y).abs() < f64::EPSILON);
        assert!((u.width - 10.0).abs() < f64::EPSILON);
        assert!((u.height - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn iou() {
        let a = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        assert!((a.iou(&b) - 1.0).abs() < f64::EPSILON);

        let c = BoundingBox::new(20.0, 20.0, 10.0, 10.0);
        assert!(a.iou(&c).abs() < f64::EPSILON);
    }

    #[test]
    fn to_u32_rounds() {
        let bb = BoundingBox::new(1.4, 2.6, 3.5, 4.4);
        let u = bb.to_u32();
        assert_eq!(u.x, 1);
        assert_eq!(u.y, 3);
        assert_eq!(u.width, 4);
        assert_eq!(u.height, 4);
    }
}
