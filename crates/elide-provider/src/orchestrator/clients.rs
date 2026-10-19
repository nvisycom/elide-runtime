//! Backend clients that cannot be built when a request compiles.
//!
//! Most backends are built per request, inside the synchronous
//! orchestrator compile: a base URL and a model name are enough, so
//! constructing one costs nothing worth caching.
//!
//! Google Cloud Document AI is not like that. Its client builder is
//! `async` — it resolves Application Default Credentials, which may
//! mean a metadata-server round trip — and returns a `Result`. That
//! cannot happen inside a synchronous per-request compile, and it
//! should not happen per request even if it could.
//!
//! So a deployment naming one of these is built through
//! [`Provider::try_new`], which awaits each client once at startup
//! and holds them here, keyed by the component name the deployment
//! gave. The per-request compile then looks one up instead of
//! building it.
//!
//! [`Provider::try_new`]: super::Provider::try_new

use std::collections::BTreeMap;

use elide::Result;
// Used only by the async-client backends below, which are the only
// code here that can fail.
#[cfg(feature = "google-document-ai")]
use elide::{Error, ErrorKind};
use elide_deployment::component::Enrichers;
#[cfg(feature = "google-document-ai")]
use elide_deployment::component::OcrBackend;
#[cfg(feature = "google-document-ai")]
use elide_google_document_ai::DocumentAiOcr;

/// The pre-built clients a provider holds, by component name.
///
/// Empty for every deployment that names no such backend, which is
/// the common case: the map is allocated but never populated, and
/// the per-request lookup never runs.
///
/// `Debug` is written by hand rather than derived: the SDK clients
/// held here do not implement it, and a deployment's debug output
/// wants the component names anyway, not a client's internals.
#[derive(Default)]
pub(crate) struct Clients {
    /// Document AI enrichers, by the component name that configured
    /// them.
    #[cfg(feature = "google-document-ai")]
    pub(crate) document_ai: BTreeMap<String, DocumentAiOcr>,
    /// Keeps the struct inhabited when no such backend is compiled
    /// in, so the field list is never empty.
    #[cfg(not(feature = "google-document-ai"))]
    pub(crate) _empty: BTreeMap<String, ()>,
}

impl std::fmt::Debug for Clients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = f.debug_struct("Clients");
        #[cfg(feature = "google-document-ai")]
        out.field("document_ai", &self.document_ai.keys().collect::<Vec<_>>());
        out.finish()
    }
}

impl Clients {
    /// Await every client the deployment's enricher lineups need.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](elide::ErrorKind::Configuration) if
    /// a client cannot be built — bad credentials, an unreachable
    /// metadata server, a malformed processor name. Failing at
    /// startup is the point: the alternative is a deployment that
    /// accepts documents and then fails every one that needs OCR.
    #[cfg_attr(
        not(feature = "google-document-ai"),
        expect(
            clippy::unused_async,
            reason = "the signature is the same whether or not an async backend is compiled in, \
so a host's call site does not change with the feature set"
        )
    )]
    pub(crate) async fn build(enrichers: &Enrichers) -> Result<Self> {
        #[cfg_attr(
            not(feature = "google-document-ai"),
            expect(unused_mut, reason = "only an async backend populates this")
        )]
        let mut clients = Self::default();

        #[cfg(feature = "google-document-ai")]
        for component in &enrichers.ocr {
            if let OcrBackend::GoogleDocumentAi {
                processor,
                endpoint,
            } = &component.backend
            {
                let name = component.name.as_str().to_owned();
                // Refuse a repeat name rather than letting the map
                // overwrite. `Deployment::validate` already rejects
                // a duplicated lineup name, but this map's
                // correctness depends on uniqueness: silently
                // overwriting would run OCR against the *other*
                // component's processor — a different model
                // version, region, possibly trust boundary — with
                // no error anywhere.
                if clients.document_ai.contains_key(&name) {
                    return Err(Error::new(
                        ErrorKind::Configuration,
                        format!(
                            "enrichers.ocr: two components are both called `{name}`, so one would run against the other's processor. Give each a unique name."
                        ),
                    ));
                }
                clients.document_ai.insert(
                    name,
                    document_ai_client(processor, endpoint.as_deref()).await?,
                );
            }
        }

        #[cfg(not(feature = "google-document-ai"))]
        let _ = enrichers;

        Ok(clients)
    }
}

/// Build one Document AI client against `processor`.
#[cfg(feature = "google-document-ai")]
async fn document_ai_client(processor: &str, endpoint: Option<&str>) -> Result<DocumentAiOcr> {
    use elide_google_document_ai::document_ai::client::DocumentProcessorService;

    let mut builder = DocumentProcessorService::builder();
    if let Some(endpoint) = endpoint {
        builder = builder.with_endpoint(endpoint.to_owned());
    }
    let client = builder.build().await.map_err(|err| {
        Error::new(
            ErrorKind::Configuration,
            format!(
                "enrichers.ocr: could not build a Document AI client for processor \
`{processor}`: {err}"
            ),
        )
    })?;
    Ok(DocumentAiOcr::new(client, processor.to_owned()))
}
