//! [`SuppressionLayer`]: drop entities whose matched text is on a
//! caller-supplied allow list.
//!
//! See [`SuppressionParams`] for the three allow-list shapes.
//!
//! All three operate on the **entity's resolved text** (sliced from
//! the source via [`TextAt::text_at`]), not the surrounding
//! document. When the resolver returns `None` (e.g. malformed
//! location), the entity is kept — better to surface a false
//! positive than silently drop something we can't verify.
//!
//! Returns dropped entities from [`Layer::apply`] so the pipeline
//! can attribute them in its drop-reason roll-up.
//!
//! [`Layer::apply`]: super::layer::Layer::apply
//! [`TextAt::text_at`]: nvisy_core::extraction::TextAt::text_at

mod params;

use nvisy_core::Error;
use nvisy_core::entity::Entity;
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::Modality;
use regex::Regex;

pub use self::params::SuppressionParams;
use super::layer::{Layer, LayerContext};

const TARGET: &str = "nvisy_toolkit::deduplication::suppress";

/// [`Layer`] that drops entities whose resolved text is on a
/// caller-supplied allow list.
///
/// Construct via [`SuppressionLayer::new`] (empty, fast no-op) or
/// [`SuppressionLayer::from_params`] (pre-validates the regex
/// inputs). An empty layer short-circuits in
/// [`Layer::apply`] without touching the resolver.
#[derive(Debug, Clone, Default)]
pub struct SuppressionLayer {
    /// Pre-lowercased exact-match values.
    allow_values: Vec<String>,
    /// Pre-lowercased substring values.
    allow_values_substring: Vec<String>,
    /// Pre-compiled regex patterns.
    allow_values_regex: Vec<Regex>,
}

impl SuppressionLayer {
    /// Empty layer: passes every entity through unchanged.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct from a [`SuppressionParams`]. Each regex source
    /// is compiled once here.
    ///
    /// Empty strings are silently dropped at construction from
    /// all three lists: each would match every entity (or every
    /// position) and is virtually never what the author meant.
    /// Treating them as configuration mistakes and ignoring them
    /// is safer than wiping every result.
    ///
    /// # Errors
    ///
    /// Returns a validation error when any non-empty entry in
    /// [`SuppressionParams::allow_values_regex`] is not a valid
    /// regular expression.
    pub fn from_params(params: &SuppressionParams) -> Result<Self, Error> {
        let allow_values = params
            .allow_values
            .iter()
            .filter(|v| !v.is_empty())
            .map(|v| v.to_ascii_lowercase())
            .collect();
        let allow_values_substring = params
            .allow_values_substring
            .iter()
            .filter(|v| !v.is_empty())
            .map(|v| v.to_ascii_lowercase())
            .collect();
        let allow_values_regex = params
            .allow_values_regex
            .iter()
            .filter(|src| !src.is_empty())
            .map(|src| {
                Regex::new(src).map_err(|e| {
                    Error::validation(
                        format!("invalid allow_values_regex `{src}`: {e}"),
                        "nvisy-toolkit",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            allow_values,
            allow_values_substring,
            allow_values_regex,
        })
    }

    /// Return `true` when no allow-list values are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allow_values.is_empty()
            && self.allow_values_substring.is_empty()
            && self.allow_values_regex.is_empty()
    }

    /// Return `true` when `text` matches any configured allow-list
    /// entry under exact / substring / regex semantics.
    #[must_use]
    pub fn suppresses(&self, text: &str) -> bool {
        let lowered = text.to_ascii_lowercase();
        if self.allow_values.iter().any(|v| v == &lowered) {
            return true;
        }
        if self
            .allow_values_substring
            .iter()
            .any(|v| lowered.contains(v.as_str()))
        {
            return true;
        }
        if self.allow_values_regex.iter().any(|r| r.is_match(text)) {
            return true;
        }
        false
    }
}

#[async_trait::async_trait]
impl<M, R> Layer<M, R> for SuppressionLayer
where
    M: Modality,
    R: TextAt<M> + ?Sized,
{
    async fn apply(
        &self,
        entities: &mut Vec<Entity<M>>,
        ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>> {
        if self.is_empty() || entities.is_empty() {
            return Vec::new();
        }

        let mut suppressed_flags = Vec::with_capacity(entities.len());
        for entity in entities.iter() {
            let suppress = match ctx.resolver.text_at(&entity.location).await {
                Some(text) => self.suppresses(&text),
                None => false,
            };
            suppressed_flags.push(suppress);
        }

        let mut suppressed_count = 0usize;
        let mut dropped = Vec::new();
        let mut idx = 0usize;
        entities.retain(|entity| {
            let drop = suppressed_flags[idx];
            idx += 1;
            if drop {
                suppressed_count += 1;
                dropped.push(entity.clone());
            }
            !drop
        });

        if suppressed_count > 0 {
            tracing::debug!(
                target: TARGET,
                suppressed = suppressed_count,
                "entities suppressed by allow list",
            );
        }
        dropped
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_core::entity::{Entity, builtins};
    use nvisy_core::modality::{Text, TextLocation};

    use super::*;

    /// Test resolver that resolves locations to a slice of a known
    /// string. The Noop test_resolver in the parent module returns
    /// `None`, which is fine for layers that don't touch text but
    /// useless here.
    struct TextSliceResolver {
        text: Arc<String>,
    }

    #[async_trait::async_trait]
    impl TextAt<Text> for TextSliceResolver {
        async fn text_at(&self, location: &TextLocation) -> Option<String> {
            self.text
                .get(location.start..location.end)
                .map(String::from)
        }
    }

    fn entity(start: usize, end: usize) -> Entity<Text> {
        Entity::test_builder(start, end)
            .with_label(builtins::EMAIL_ADDRESS.label_ref())
            .test_build()
    }

    fn params(values: &[&str], substrings: &[&str], regexes: &[&str]) -> SuppressionParams {
        SuppressionParams {
            allow_values: values.iter().map(|s| (*s).to_owned()).collect(),
            allow_values_substring: substrings.iter().map(|s| (*s).to_owned()).collect(),
            allow_values_regex: regexes.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    async fn apply_to(
        layer: &SuppressionLayer,
        source: &str,
        mut entities: Vec<Entity<Text>>,
    ) -> (Vec<Entity<Text>>, Vec<Entity<Text>>) {
        let resolver = TextSliceResolver {
            text: Arc::new(source.to_owned()),
        };
        let ctx = LayerContext::new(&resolver);
        let dropped = layer.apply(&mut entities, &ctx).await;
        (entities, dropped)
    }

    #[tokio::test]
    async fn empty_layer_is_noop() {
        let layer = SuppressionLayer::new();
        let source = "noreply@foo.com matters";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(0, 15)]).await;
        assert_eq!(kept.len(), 1);
        assert!(dropped.is_empty());
    }

    #[tokio::test]
    async fn exact_match_drops_entity() {
        let layer = SuppressionLayer::from_params(&params(&["noreply@foo.com"], &[], &[]))
            .expect("layer builds");
        let source = "noreply@foo.com matters";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(0, 15)]).await;
        assert!(kept.is_empty());
        assert_eq!(dropped.len(), 1);
    }

    #[tokio::test]
    async fn exact_match_is_case_insensitive() {
        let layer = SuppressionLayer::from_params(&params(&["NoReply@Foo.com"], &[], &[]))
            .expect("layer builds");
        let source = "noreply@foo.com matters";
        let (kept, _) = apply_to(&layer, source, vec![entity(0, 15)]).await;
        assert!(kept.is_empty(), "case-insensitive allow-list should drop");
    }

    #[tokio::test]
    async fn exact_match_does_not_drop_partial_overlap() {
        // Allow value is a substring of the entity, but not an
        // exact equal — exact mode keeps it.
        let layer = SuppressionLayer::from_params(&params(&["noreply@foo.com"], &[], &[]))
            .expect("layer builds");
        let source = "noreply@foo.com support team";
        let (kept, _) = apply_to(&layer, source, vec![entity(0, 28)]).await;
        assert_eq!(kept.len(), 1, "exact mode must not drop on partial overlap");
    }

    #[tokio::test]
    async fn substring_match_drops_partial_overlap() {
        let layer = SuppressionLayer::from_params(&params(&[], &["noreply@foo.com"], &[]))
            .expect("layer builds");
        let source = "noreply@foo.com support team";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(0, 28)]).await;
        assert!(kept.is_empty());
        assert_eq!(dropped.len(), 1);
    }

    #[tokio::test]
    async fn regex_match_drops_entity() {
        let layer = SuppressionLayer::from_params(&params(&[], &[], &[r"^test-.*@foo\.com$"]))
            .expect("layer builds");
        let source = "test-1234@foo.com";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(0, source.len())]).await;
        assert!(kept.is_empty());
        assert_eq!(dropped.len(), 1);
    }

    #[tokio::test]
    async fn invalid_regex_at_construction_errors() {
        let result = SuppressionLayer::from_params(&params(&[], &[], &["["]));
        assert!(result.is_err(), "invalid regex must error at construction");
    }

    #[tokio::test]
    async fn unresolved_text_keeps_entity() {
        // Pass an entity with a location outside the source text.
        // text_at returns None, the layer falls open and keeps the
        // entity rather than silently dropping it.
        let layer = SuppressionLayer::from_params(&params(&["noreply@foo.com"], &[], &[]))
            .expect("layer builds");
        let source = "short";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(100, 200)]).await;
        assert_eq!(kept.len(), 1);
        assert!(dropped.is_empty());
    }

    #[tokio::test]
    async fn empty_substring_entry_does_not_suppress_everything() {
        // `str::contains("")` is always true; without the
        // construction-time filter, an empty entry would wipe
        // every match. Confirm the filter holds.
        let layer = SuppressionLayer::from_params(&params(&[], &[""], &[])).expect("layer builds");
        let source = "noreply@foo.com matters";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(0, 15)]).await;
        assert_eq!(kept.len(), 1, "empty substring must not drop");
        assert!(dropped.is_empty());
    }

    #[tokio::test]
    async fn empty_exact_entry_is_ignored() {
        // An empty exact entry could only match an empty entity,
        // which recognizers don't emit. Filtering it costs
        // nothing and keeps the lookup short.
        let layer = SuppressionLayer::from_params(&params(&[""], &[], &[])).expect("layer builds");
        let source = "noreply@foo.com matters";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(0, 15)]).await;
        assert_eq!(kept.len(), 1);
        assert!(dropped.is_empty());
    }

    #[tokio::test]
    async fn empty_regex_entry_is_ignored() {
        // An empty regex matches at every position. Same
        // catastrophe as empty substring; filter at construction.
        let layer = SuppressionLayer::from_params(&params(&[], &[], &[""])).expect("layer builds");
        let source = "noreply@foo.com matters";
        let (kept, dropped) = apply_to(&layer, source, vec![entity(0, 15)]).await;
        assert_eq!(kept.len(), 1, "empty regex must not drop");
        assert!(dropped.is_empty());
    }

    #[tokio::test]
    async fn union_across_modes() {
        // Three allow-list shapes, three entities. Each entity is
        // suppressed by exactly one mode; all three drop.
        let layer = SuppressionLayer::from_params(&params(
            &["alpha@x.com"],
            &["bravo"],
            &[r"^charlie-\d+$"],
        ))
        .expect("layer builds");
        let source = "alpha@x.com bravo-team-12 charlie-99";
        let entities = vec![
            entity(0, 11),  // exact
            entity(12, 25), // substring
            entity(26, 36), // regex
        ];
        let (kept, dropped) = apply_to(&layer, source, entities).await;
        assert!(kept.is_empty(), "all three should be suppressed");
        assert_eq!(dropped.len(), 3);
    }
}
