//! Envelope transport: fan-in, fan-out, and cloning for pipeline DAG edges.
//!
//! These functions manage [`DocumentEnvelope`] flow between pipeline
//! nodes via bounded MPSC channels. They are used by the [executor]
//! but live here because they are a transport concern, not an
//! operation-dispatch concern.
//!
//! [executor]: super::executor

use std::future::Future;

use futures::StreamExt;
use nvisy_core::Error;
use tokio::sync::mpsc;

use crate::operation::DocumentEnvelope;

/// Core envelope processing loop shared by most node types.
///
/// Merges all upstream receivers concurrently (true fan-in), applies
/// `transform` to each envelope, and fans out the result to all
/// downstream senders. Returns the total number of envelopes processed.
pub(super) async fn process_envelopes<F, Fut>(
    senders: &[mpsc::Sender<DocumentEnvelope>],
    receivers: &mut [mpsc::Receiver<DocumentEnvelope>],
    mut transform: F,
) -> Result<u64, Error>
where
    F: FnMut(DocumentEnvelope) -> Fut,
    Fut: Future<Output = Result<DocumentEnvelope, Error>>,
{
    let mut count = 0u64;

    if receivers.len() <= 1 {
        // Fast path: single receiver, no merging needed.
        if let Some(rx) = receivers.first_mut() {
            while let Some(envelope) = rx.recv().await {
                let envelope = transform(envelope).await?;
                count += 1;
                fan_out(senders, envelope).await?;
            }
        }
    } else {
        // Concurrent fan-in: merge all receivers into a single stream
        // so slow upstreams don't block fast ones.
        let streams: Vec<_> = receivers
            .iter_mut()
            .map(|rx| {
                let owned = {
                    let (_, mut placeholder) = mpsc::channel(1);
                    std::mem::swap(rx, &mut placeholder);
                    placeholder
                };
                Box::pin(futures::stream::unfold(owned, |mut rx| async move {
                    rx.recv().await.map(|item| (item, rx))
                }))
                    as std::pin::Pin<Box<dyn futures::Stream<Item = DocumentEnvelope> + Send>>
            })
            .collect();
        let mut merged = futures::stream::select_all(streams);

        while let Some(envelope) = StreamExt::next(&mut merged).await {
            let envelope = transform(envelope).await?;
            count += 1;
            fan_out(senders, envelope).await?;
        }
    }

    Ok(count)
}

/// Send an envelope to a single downstream sender.
///
/// Fan-out to multiple senders is not supported because
/// [`DocumentEnvelope`] is intentionally non-cloneable. Graphs that
/// require branching should use separate import nodes instead.
///
/// # Errors
///
/// Returns a runtime error if the downstream channel is closed or if
/// more than one sender is provided.
pub(super) async fn fan_out(
    senders: &[mpsc::Sender<DocumentEnvelope>],
    envelope: DocumentEnvelope,
) -> Result<(), Error> {
    match senders {
        [] => Ok(()),
        [tx] => tx
            .send(envelope)
            .await
            .map_err(|_| Error::runtime("downstream channel closed", "transport", false)),
        _ => Err(Error::runtime(
            "fan-out to multiple downstream channels is not supported; \
             DocumentEnvelope cannot be cloned",
            "transport",
            false,
        )),
    }
}

/// Drain all receivers and forward envelopes to all senders unchanged.
///
/// Used in dry-run mode to pass envelopes through skipped nodes so
/// downstream watch channels still unblock.
pub(super) async fn forward_envelopes(
    senders: &[mpsc::Sender<DocumentEnvelope>],
    receivers: &mut [mpsc::Receiver<DocumentEnvelope>],
) -> Result<u64, Error> {
    let mut count = 0u64;
    for rx in receivers.iter_mut() {
        while let Some(envelope) = rx.recv().await {
            fan_out(senders, envelope).await?;
            count += 1;
        }
    }
    Ok(count)
}

