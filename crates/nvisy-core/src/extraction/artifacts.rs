//! [`Artifacts`]: heterogeneous typed-map newtype attached to a
//! [`Span<M>`] so extractors (and any future stage) can read or
//! stamp out-of-band enrichments alongside the payload without
//! widening the call surface for new entry types.
//!
//! Keyed by Rust type, so each entry is a distinct typed slot.
//! Backed by [`type_map::concurrent::TypeMap`] today; the newtype
//! exists so consumer code never names the underlying crate and we
//! can swap representations later without churn.
//!
//! [`Span<M>`]: crate::extraction::Span

use std::fmt;

use type_map::concurrent::TypeMap;

/// Heterogeneous typed-map of per-span artifacts.
///
/// Entries are keyed by their concrete Rust type; only one value of
/// any given type can be present at a time. Consumers do
/// `artifacts.get::<T>()` / `artifacts.insert(value)`; when the entry
/// is absent the consumer silently degrades.
///
/// `Send + Sync` so spans cross thread boundaries; cloning is *not*
/// supported (entries may not themselves be `Clone`), matching
/// `TypeMap`'s own surface.
#[derive(Default)]
pub struct Artifacts(TypeMap);

impl Artifacts {
    /// Build an empty bundle.
    pub fn new() -> Self {
        Self(TypeMap::new())
    }

    /// Insert one typed entry. Returns the previous value of that
    /// type if one was already present.
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) -> Option<T> {
        self.0.insert(value)
    }

    /// Borrow the entry of type `T`, if present.
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.0.get::<T>()
    }

    /// Remove and return the entry of type `T`, if present.
    pub fn take<T: Send + Sync + 'static>(&mut self) -> Option<T> {
        self.0.remove::<T>()
    }

    /// Whether an entry of type `T` is present.
    pub fn contains<T: Send + Sync + 'static>(&self) -> bool {
        self.0.contains::<T>()
    }
}

impl fmt::Debug for Artifacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Artifacts").finish_non_exhaustive()
    }
}
