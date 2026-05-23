//! Private `Progress` implementation used by [`Downloader::fetch`].
//!
//! Not re-exported — every fetch wires it in implicitly. Kept in its
//! own file so the downloader stays focused on transfer orchestration.
//!
//! [`Downloader::fetch`]: super::Downloader::fetch

use hf_hub::api::tokio::Progress;

/// Emits structured `tracing::trace` events throttled to ~5% intervals.
///
/// Created fresh per `fetch` call; not exposed publicly because every
/// call uses it implicitly.
#[derive(Debug, Default, Clone)]
pub(super) struct TracingProgress {
    filename: String,
    total: usize,
    downloaded: usize,
    /// Threshold (in bytes) at which the next progress line fires.
    next_log_at: usize,
}

impl Progress for TracingProgress {
    async fn init(&mut self, size: usize, filename: &str) {
        self.filename = filename.to_owned();
        self.total = size;
        self.downloaded = 0;
        // Aim for ~20 progress lines per download.
        self.next_log_at = size / 20;
        tracing::trace!(
            target: "nvisy_core::hf",
            filename = %self.filename,
            total_bytes = size,
            "download started"
        );
    }

    async fn update(&mut self, bytes: usize) {
        self.downloaded += bytes;
        if self.total > 0 && self.downloaded >= self.next_log_at {
            let pct = (self.downloaded * 100) / self.total.max(1);
            tracing::trace!(
                target: "nvisy_core::hf",
                filename = %self.filename,
                downloaded_bytes = self.downloaded,
                total_bytes = self.total,
                percent = pct,
                "download progress"
            );
            self.next_log_at = self.downloaded + (self.total / 20);
        }
    }

    async fn finish(&mut self) {
        tracing::trace!(
            target: "nvisy_core::hf",
            filename = %self.filename,
            total_bytes = self.total,
            "download complete"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tracing_progress_lifecycle_compiles() {
        let mut p = TracingProgress::default();
        p.init(1_000, "model.onnx").await;
        p.update(500).await;
        p.update(500).await;
        p.finish().await;
    }
}
