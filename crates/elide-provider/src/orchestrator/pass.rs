//! [`Pass`]: one pass over one document, as the provider compiles it.

use elide::Result;
use elide::entity::LabelCatalog;
use elide::recognition::Scope;
use elide_schema::compile_catalog;
use elide_schema::policy::Policy;
use elide_schema::recognition::Recognition;
use uuid::Uuid;

use super::DocumentContext;
use super::validate::validate_scope_references;

/// One pass over one document: the policies it applies, and what
/// the caller asserted about the document.
///
/// What every per-request [`Provider`](super::Provider) method
/// compiles against. Analyze builds one from the request; anonymize
/// from the audit analyze wrote, so both passes compile the same
/// vocabulary.
#[derive(Debug, Clone, Copy)]
pub struct Pass<'a> {
    /// The policies this pass applies. Their scopes union into the
    /// label catalog that drives detection and selection.
    pub policies: &'a [Policy],
    /// What the caller asserts about the document: languages,
    /// jurisdictions, free-form classification.
    pub context: &'a DocumentContext,
    /// Vocabularies the request introduces beyond the shipped set.
    pub recognition: &'a [Recognition],
    /// The document's id, tagging the pass's tracing spans.
    pub correlation_id: Uuid,
}

impl Pass<'_> {
    /// The label catalog this pass detects and selects against.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](elide::ErrorKind::Configuration)
    /// if a rule references a scope its policy never declared, a
    /// policy declares one twice, or a label does not resolve.
    pub(crate) fn catalog(&self) -> Result<LabelCatalog> {
        validate_scope_references(self.policies)?;
        compile_catalog(self.policies, self.recognition)
    }

    /// The elide [`Scope`] this pass runs under: the caller's
    /// assertions, `catalog`, and the correlation id.
    pub(crate) fn scope(&self, catalog: LabelCatalog) -> Scope {
        let context = self.context;
        Scope::new()
            .with_languages(context.languages.iter().cloned())
            .with_countries(context.countries.iter().copied())
            .with_metadata(context.metadata.clone())
            .with_catalog(catalog)
            .with_correlation_id(self.correlation_id)
    }
}
