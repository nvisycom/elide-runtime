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

pub mod au;
pub mod ca;
pub mod de;
pub mod es;
pub mod fi;
pub mod it;
pub mod pl;
pub mod se;
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
    out.extend(de::all());
    out.extend(es::all());
    out.extend(it::all());
    out.extend(pl::all());
    out.extend(au::all());
    out.extend(ca::all());
    out.extend(fi::all());
    out.extend(se::all());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_pattern_has_variants() {
        for pattern in all() {
            assert!(
                !pattern.variants.is_empty(),
                "pattern `{}` has no variants",
                pattern.name,
            );
        }
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
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
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
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["GB"],
                "UK-scoped pattern `{}` must declare countries = [GB]",
                pattern.name,
            );
        }
    }

    #[test]
    fn de_patterns_are_country_scoped_to_de() {
        for pattern in de::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["DE"],
                "DE-scoped pattern `{}` must declare countries = [DE]",
                pattern.name,
            );
        }
    }

    #[test]
    fn es_patterns_are_country_scoped_to_es() {
        for pattern in es::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["ES"],
                "ES-scoped pattern `{}` must declare countries = [ES]",
                pattern.name,
            );
        }
    }

    #[test]
    fn it_patterns_are_country_scoped_to_it() {
        for pattern in it::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["IT"],
                "IT-scoped pattern `{}` must declare countries = [IT]",
                pattern.name,
            );
        }
    }

    #[test]
    fn pl_patterns_are_country_scoped_to_pl() {
        for pattern in pl::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["PL"],
                "PL-scoped pattern `{}` must declare countries = [PL]",
                pattern.name,
            );
        }
    }

    #[test]
    fn au_patterns_are_country_scoped_to_au() {
        for pattern in au::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["AU"],
                "AU-scoped pattern `{}` must declare countries = [AU]",
                pattern.name,
            );
        }
    }

    #[test]
    fn ca_patterns_are_country_scoped_to_ca() {
        for pattern in ca::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["CA"],
                "CA-scoped pattern `{}` must declare countries = [CA]",
                pattern.name,
            );
        }
    }

    #[test]
    fn fi_patterns_are_country_scoped_to_fi() {
        for pattern in fi::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["FI"],
                "FI-scoped pattern `{}` must declare countries = [FI]",
                pattern.name,
            );
        }
    }

    #[test]
    fn se_patterns_are_country_scoped_to_se() {
        for pattern in se::all() {
            assert_eq!(
                pattern
                    .countries
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                vec!["SE"],
                "SE-scoped pattern `{}` must declare countries = [SE]",
                pattern.name,
            );
        }
    }
}
