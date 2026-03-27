//! Context node configurations: load, generate, and save.
//!
//! Context nodes bookend the pipeline: [`LoadContext`] runs at **phase 0**
//! to pull reference data into the envelope before any processing begins,
//! [`GenerateContext`] runs at **phase 4** to synthesise a new context entry
//! from detection results, and [`SaveContext`] runs at **phase 6** to persist
//! selected contexts back to the registry.

mod generate;
mod load;
mod save;

pub use self::generate::GenerateContext;
pub use self::load::LoadContext;
pub use self::save::SaveContext;
