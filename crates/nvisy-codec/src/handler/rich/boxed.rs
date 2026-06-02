//! [`RichHandle`]: marker trait combining [`Handle<Text>`] and
//! [`Handle<Image>`] so a single trait object can hold either view
//! of a rich (text + image) document like a PDF.
//!
//! Rich-format handlers (e.g. the PDF handler in `nvisy-formats`)
//! implement [`Handle<Text>`] and [`Handle<Image>`] directly on
//! their type; the blanket impl on [`RichHandle`] picks them up
//! automatically. The [`DocumentHandle::Rich`] variant carries a
//! `Box<dyn RichHandle>` so the engine can dispatch to either
//! modality through one trait object.
//!
//! [`Handle<Text>`]: crate::core::Handle
//! [`Handle<Image>`]: crate::core::Handle
//! [`DocumentHandle::Rich`]: crate::document::DocumentHandle::Rich

use nvisy_core::modality::{Image, Text};

use crate::core::Handle;

/// Marker trait: a handler that exposes both [`Text`] and [`Image`]
/// modalities, addressable through a single trait object.
pub trait RichHandle: Handle<Text> + Handle<Image> {}

impl<T> RichHandle for T where T: Handle<Text> + Handle<Image> {}
