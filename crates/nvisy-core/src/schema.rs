//! Wire-format proxies for elide-core types embedded in nvisy-core
//! shapes, plus round-trip [`From`] / [`Into`] conversions to bridge
//! them back to the elide vocabulary at request time.
//!
//! Elide-core stays free of `schemars` — schema generation is an
//! HTTP-layer concern the toolkit doesn't model. These proxies *are*
//! the wire shape nvisy-core types use; engine converts at the seam
//! when it needs an elide-typed value.
//!
//! Naming: each proxy is `<EliteName>Schema` (e.g. `LabelSchema`
//! mirrors `elide_core::entity::Label`). The proxies derive
//! [`schemars::JsonSchema`] plus the serde + clone basics; the
//! [`From`] impls do round-trip conversions to / from their elide
//! counterparts.

use elide_core::entity::Label;
use elide_core::primitive::{BoundingBox, LanguageTag, Point, Polygon, TimeSpan};
use elide_core::redaction::OperatorId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wire-shape proxy for [`elide_core::entity::Label`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Label")]
pub struct LabelSchema {
    /// Stable identifier, e.g. `"email_address"`.
    pub name: String,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Free-form tags policy selectors can target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl From<LabelSchema> for Label {
    fn from(s: LabelSchema) -> Self {
        let label = match s.description {
            Some(desc) => Label::described(s.name, desc),
            None => Label::new(s.name),
        };
        if s.tags.is_empty() {
            label
        } else {
            label.with_tags(s.tags)
        }
    }
}

impl From<Label> for LabelSchema {
    fn from(l: Label) -> Self {
        Self {
            name: l.name().to_owned(),
            description: l.description().map(str::to_owned),
            tags: l.tags().iter().map(|t| t.as_str().to_owned()).collect(),
        }
    }
}

/// Wire-shape proxy for [`elide_core::redaction::OperatorId`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "OperatorId")]
pub struct OperatorIdSchema {
    /// Stable operator name (e.g. `"mask"`, `"aes-gcm-encrypt"`).
    pub name: String,
    /// Operator version at the time it was applied.
    pub version: String,
}

impl From<OperatorIdSchema> for OperatorId {
    fn from(s: OperatorIdSchema) -> Self {
        OperatorId::new(s.name, s.version)
    }
}

impl From<OperatorId> for OperatorIdSchema {
    fn from(o: OperatorId) -> Self {
        Self {
            name: o.name.as_str().to_owned(),
            version: o.version.as_str().to_owned(),
        }
    }
}

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

/// Wire-shape proxy for [`elide_core::primitive::TimeSpan`].
///
/// Microsecond half-open interval `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "TimeSpan")]
pub struct TimeSpanSchema {
    /// Start of the interval, microseconds from the stream start.
    pub start_us: u64,
    /// End of the interval (exclusive), microseconds from the stream
    /// start.
    pub end_us: u64,
}

impl From<TimeSpanSchema> for TimeSpan {
    fn from(s: TimeSpanSchema) -> Self {
        TimeSpan::new(s.start_us, s.end_us)
    }
}

impl From<TimeSpan> for TimeSpanSchema {
    fn from(t: TimeSpan) -> Self {
        Self {
            start_us: t.start_micros(),
            end_us: t.end_micros(),
        }
    }
}

/// Wire-shape proxy for [`elide_core::primitive::LanguageTag`].
///
/// A BCP 47 tag string (e.g. `"en"`, `"de-CH"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(rename = "LanguageTag", transparent)]
pub struct LanguageTagSchema(pub String);

impl TryFrom<LanguageTagSchema> for LanguageTag {
    type Error = elide_core::Error;
    fn try_from(s: LanguageTagSchema) -> Result<Self, Self::Error> {
        LanguageTag::parse(s.0)
            .map_err(|e| elide_core::Error::new(elide_core::ErrorKind::Validation, e))
    }
}

impl From<LanguageTag> for LanguageTagSchema {
    fn from(t: LanguageTag) -> Self {
        Self(t.as_str().to_owned())
    }
}
