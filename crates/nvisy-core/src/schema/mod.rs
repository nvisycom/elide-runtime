//! Wire-format proxies for elide-core types embedded in nvisy-core
//! shapes, plus round-trip [`From`] / [`Into`] conversions to bridge
//! them back to the elide vocabulary at request time.
//!
//! Elide-core stays free of `schemars` — schema generation is an
//! HTTP-layer concern the toolkit doesn't model. These proxies *are*
//! the wire shape nvisy-core types use; engine converts at the seam
//! when it needs an elide-typed value.
//!
//! Each proxy is named `<ElideName>Schema` (e.g. [`LabelSchema`]
//! mirrors [`elide_core::entity::Label`]). The proxies derive
//! [`schemars::JsonSchema`] plus serde + clone basics; their
//! conversions are pure [`From`] / [`Into`] (or [`TryFrom`] when the
//! elide constructor is fallible).

mod color;
mod geometry;
mod label;
mod language;
mod operator;
mod time;
mod waveform;

pub use self::color::ColorSchema;
pub use self::geometry::{BoundingBoxSchema, PointSchema, PolygonSchema};
pub use self::label::LabelSchema;
pub use self::language::LanguageTagSchema;
pub use self::operator::OperatorIdSchema;
pub use self::time::TimeSpanSchema;
pub use self::waveform::WaveformSchema;
