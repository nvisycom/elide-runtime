//! Built-in [`Regex`] rules, embedded at compile time.
//!
//! Accessors are grouped by region — `world::*` for universal
//! patterns, `<country>::*` (e.g. `us::*`, `uk::*`) for
//! country-specific ones. Each returns a fresh [`Regex`] parsed
//! from a TOML definition under
//! `assets/patterns/<region>/<domain>/`. The parse happens on
//! every call — rules are cheap to construct since
//! [`PatternRecognizer::build`] does the heavy compilation.
//!
//! [`Regex`]: crate::Regex
//! [`PatternRecognizer::build`]: crate::PatternRecognizer

pub mod uk;
pub mod us;
pub mod world;

use crate::Regex;

/// Helper used by every per-region sub-module to define a shipped
/// pattern accessor.
///
/// The `$path` is resolved with `include_str!` against the path
/// of the file that *expands* the macro, so callers in sub-modules
/// (e.g. `world.rs`) pass paths relative to themselves.
#[doc(hidden)]
#[macro_export]
macro_rules! __shipped_pattern {
    ($(#[$meta:meta])* fn $name:ident from $path:literal) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name() -> $crate::Regex {
            $crate::Regex::from_toml(include_str!($path))
                .expect(concat!("shipped pattern `", $path, "` is well-formed"))
        }
    };
}

/// Every built-in pattern shipped by this crate, regardless of
/// region.
#[must_use]
pub fn all() -> Vec<Regex> {
    let mut out = world::all();
    out.extend(us::all());
    out.extend(uk::all());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_pattern_parses() {
        let patterns = all();
        assert_eq!(patterns.len(), 34);
    }

    #[test]
    fn world_set_has_18_patterns() {
        assert_eq!(world::all().len(), 18);
    }

    #[test]
    fn us_set_has_10_patterns() {
        assert_eq!(us::all().len(), 10);
    }

    #[test]
    fn uk_set_has_6_patterns() {
        assert_eq!(uk::all().len(), 6);
    }

    #[test]
    fn world_patterns_have_no_country_scope() {
        for pattern in world::all() {
            assert!(
                pattern.countries.is_empty(),
                "world-scoped pattern `{}` must not declare countries",
                pattern.name,
            );
        }
    }

    #[test]
    fn us_patterns_are_country_scoped_to_us() {
        for pattern in us::all() {
            assert_eq!(
                pattern.countries.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                vec!["US"],
                "US-scoped pattern `{}` must declare countries = [US]",
                pattern.name,
            );
        }
    }

    #[test]
    fn uk_patterns_are_country_scoped_to_gb() {
        for pattern in uk::all() {
            assert_eq!(
                pattern.countries.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                vec!["GB"],
                "UK-scoped pattern `{}` must declare countries = [GB]",
                pattern.name,
            );
        }
    }
}
