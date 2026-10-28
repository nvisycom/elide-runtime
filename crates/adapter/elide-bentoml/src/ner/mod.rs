//! [`BentoNer`]: a [`Backend`] backed by the
//! `elide-ner` BentoML service.
//!
//! Wire contract: `POST /recognize` accepts a batched list of
//! requests (one schema-driven entity-extraction call per item)
//! and returns the matching list of responses. Each request carries
//! an entity schema. The Python models own the wire contract. Per-call
//! correlation IDs propagate as `x-request-id` headers when set.
//!
//! Wire types live in the private `request` (outgoing) and
//! `response` (incoming) submodules; only the public
//! [`BentoNer`] backend is part of this crate's API.
//!
//! [`Backend`]: elide_core::backend::Backend

mod request;
mod response;
#[allow(dead_code, reason = "Typify emits helpers not used by every schema")]
mod wire;

use std::time::Duration;

use bentoml::{Client, Endpoint};
use elide_core::Result;
use elide_core::backend::Backend;
use elide_core::entity::audit::ModelEvent;
use elide_ner::backend::{NerRequest, NerResponse};
use hipstr::HipStr;
use uuid::Uuid;

use self::wire::{NerInput, NerOutput, NerRequest as WireNerRequest};
use crate::error::BentoError;

const ROUTE: &str = "recognize";

/// BentoML NER backend.
///
/// Owns a cached [`Endpoint`] pointing at the `elide-ner`
/// `/recognize` route, plus the per-deployment model id (echoed
/// into [`Backend::provenance`]) and a default per-label
/// confidence threshold the service applies when a schema entry
/// does not pin its own.
#[derive(Debug, Clone)]
pub struct BentoNer {
    /// Pre-built endpoint at the `/recognize` route. Cloned per
    /// call so per-request headers (`x-request-id`) layer onto a
    /// fresh instance without rebuilding the route.
    /// Kept past construction so readiness can be probed: `/readyz` is a
    /// service-level endpoint, not a route, and `Endpoint` does not expose the
    /// client it was built from.
    client: Client,
    endpoint: Endpoint,
    /// Service-side model identifier echoed in provenance.
    model_id: HipStr<'static>,
    /// Default per-label confidence cutoff sent on every request.
    /// Per-label thresholds in the schema override it.
    default_threshold: f32,
}

impl BentoNer {
    /// Build from a service URL + the deployment's model id. The
    /// default per-label threshold starts at `0.5` (matches the
    /// service's own default); use [`with_default_threshold`] to
    /// override.
    ///
    /// [`with_default_threshold`]: Self::with_default_threshold
    pub fn new(base_url: impl Into<String>, model_id: impl Into<HipStr<'static>>) -> Result<Self> {
        let client = Client::builder()
            .with_base_url(base_url)
            .build()
            .map_err(BentoError::Transport)?;
        Ok(Self {
            endpoint: client.endpoint(ROUTE),
            client,
            model_id: model_id.into(),
            default_threshold: 0.5,
        })
    }

    /// Wait for the service to report itself ready, or give up after
    /// `timeout`.
    ///
    /// Construction does not connect, so the first call to a service that is
    /// still loading its model fails as a transport error — and the window is
    /// not small: these services bind the port before the model is loaded, and
    /// a restarted container spends ~9s reloading it. A caller that starts
    /// alongside the service should wait here rather than discover this one
    /// failed request at a time.
    ///
    /// Not called from [`new`](Self::new): construction stays non-async and
    /// free, so a caller that already knows the service is up pays nothing,
    /// and one that does not chooses its own deadline.
    pub async fn wait_until_ready(&self, timeout: Duration, interval: Duration) -> Result<()> {
        self.client
            .wait_until_ready(timeout, interval)
            .await
            .map_err(BentoError::Transport)?;
        Ok(())
    }

    /// Override the per-request default confidence threshold (the
    /// service applies it when a schema entity has no per-label
    /// `threshold` of its own).
    #[must_use]
    pub fn with_default_threshold(mut self, threshold: f32) -> Self {
        self.default_threshold = threshold;
        self
    }

    /// Send one batched `/recognize` POST end-to-end: encode
    /// each [`NerRequest`] to its wire form, POST the batch
    /// (layering `x-request-id` when any request carries a
    /// correlation id), decode the wire responses back to
    /// [`NerResponse`]s.
    async fn post_recognize(
        &self,
        requests: &[NerRequest<'_>],
    ) -> Result<Vec<NerResponse>, BentoError> {
        // Explicitly empty labels mean no work. Missing labels are an error:
        // this model has no implicit fixed label set.
        let mut responses: Vec<Option<NerResponse>> = requests.iter().map(|_| None).collect();
        let mut active = Vec::new();
        let mut body = Vec::new();
        for (index, request) in requests.iter().enumerate() {
            if request.labels.is_some_and(|labels| labels.is_empty()) {
                responses[index] = Some(NerResponse::new(Vec::new()));
            } else {
                body.push(WireNerRequest::from_request(
                    request,
                    self.default_threshold,
                )?);
                active.push(index);
            }
        }
        if body.is_empty() {
            return Ok(responses.into_iter().flatten().collect());
        }
        // Always sent, generated when the caller has none: the id is what ties
        // a service-side log line or rejection to the job that caused it, and
        // without one the service logs `req_id=None` for every call — which is
        // exactly when that correlation is wanted. A generated id does not
        // reach back to a caller's own job, but it still groups the service's
        // own records for one call.
        let request_id = requests
            .iter()
            .find_map(|r| r.correlation_id)
            .unwrap_or_else(Uuid::new_v4);
        let endpoint = self
            .endpoint
            .clone()
            .with_request_id(request_id.to_string());
        let wire: NerOutput = endpoint
            .invoke(&NerInput { requests: body })
            .await
            .map_err(BentoError::Transport)?;
        if wire.len() != active.len() {
            return Err(BentoError::Protocol(format!(
                "bento ner returned {} responses for {} requests",
                wire.len(),
                active.len()
            )));
        }
        for (index, response) in active.into_iter().zip(wire.0) {
            responses[index] = Some(response.decode_request(&requests[index])?);
        }
        Ok(responses.into_iter().flatten().collect())
    }
}

#[async_trait::async_trait]
impl Backend for BentoNer {
    type Request<'a> = NerRequest<'a>;
    type Response = NerResponse;

    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: self.model_id.clone(),
            version: None,
            contextual: false,
        }
    }

    async fn call(&self, request: NerRequest<'_>) -> Result<NerResponse> {
        // One request in, so exactly one response out. `pop` alone would
        // take the last of a longer list and hide the contract violation.
        let responses = self.post_recognize(&[request]).await?;
        let [response] = <[NerResponse; 1]>::try_from(responses).map_err(|responses| {
            BentoError::Protocol(format!(
                "bento ner returned {} responses for 1 request",
                responses.len(),
            ))
        })?;
        Ok(response)
    }

    /// Overridden: the `elide-ner` service takes a batch in one POST, so the
    /// whole slice goes in a single round trip rather than the default's
    /// sequential fan-out over [`call`](Self::call).
    ///
    /// Responses come back in request order. An empty batch makes no call.
    ///
    /// The slice is sent as one POST, however long it is: the service splits
    /// it into its own batch windows server-side. One POST must still finish
    /// inside the service's request timeout, so a caller coalescing a whole
    /// document wraps this in `elide::backend::ratelimit::BatchChunked` to
    /// spread it over several requests.
    async fn call_batch(&self, requests: Vec<NerRequest<'_>>) -> Result<Vec<NerResponse>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let responses = self.post_recognize(&requests).await?;
        if responses.len() != requests.len() {
            return Err(BentoError::Protocol(format!(
                "bento ner returned {} responses for {} requests",
                responses.len(),
                requests.len(),
            ))
            .into());
        }
        Ok(responses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_labels_return_no_work_without_transport() {
        let backend = BentoNer::new("http://127.0.0.1:9", "test").unwrap();
        let request = NerRequest {
            text: "Ada",
            labels: Some(&[]),
            language: None,
            correlation_id: None,
        };
        let response = backend.call(request).await.unwrap();
        assert!(response.spans.is_empty());
    }

    #[tokio::test]
    async fn missing_labels_fail_locally() {
        let backend = BentoNer::new("http://127.0.0.1:9", "test").unwrap();
        let request = NerRequest {
            text: "Ada",
            labels: None,
            language: None,
            correlation_id: None,
        };
        let error = backend.call(request).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires explicit entity labels")
        );
    }

    #[tokio::test]
    async fn mixed_batch_skips_empty_schemas_and_preserves_response_order() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        use elide_core::entity::Label;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let headers = String::from_utf8_lossy(&bytes);
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(payload["requests"].as_array().unwrap().len(), 1);
            assert_eq!(payload["requests"][0]["text"], "Łódź Ada");
            let response = r#"[{"entities":[{"text":"Ada","label":"person","score":0.9,"start":5,"end":8}],"model_id":"m"}]"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        });
        let backend = BentoNer::new(format!("http://{address}"), "test").unwrap();
        let labels = [Label::new("person_name", "person")];
        let empty = NerRequest {
            text: "ignored",
            labels: Some(&[]),
            language: None,
            correlation_id: None,
        };
        let active = NerRequest {
            text: "Łódź Ada",
            labels: Some(&labels),
            language: None,
            correlation_id: None,
        };
        let responses = backend
            .call_batch(vec![empty.clone(), active, empty])
            .await
            .unwrap();
        server.join().unwrap();
        assert_eq!(responses.len(), 3);
        assert!(responses[0].spans.is_empty());
        assert_eq!(responses[1].spans[0].offset, 8..11);
        assert_eq!(responses[1].spans[0].label, labels[0].to_ref());
        assert!(responses[2].spans.is_empty());
    }
}
