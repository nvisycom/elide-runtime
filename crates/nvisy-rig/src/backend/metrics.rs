//! Token usage tracking and statistics.

use std::sync::Mutex;

use rig::completion::Usage;

/// Tracks cumulative token usage across LLM requests.
pub struct UsageTracker {
    inner: Mutex<UsageStats>,
}

/// Snapshot of accumulated usage statistics.
#[derive(Debug, Default, Clone)]
pub struct UsageStats {
    /// Total input (prompt) tokens consumed.
    pub total_input_tokens: u64,
    /// Total output (completion) tokens consumed.
    pub total_output_tokens: u64,
    /// Total number of LLM requests sent.
    pub total_requests: u64,
    /// Total number of retries across all requests.
    pub total_retries: u64,
}

impl UsageTracker {
    /// Create a new tracker with zeroed counters.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(UsageStats::default()),
        }
    }

    /// Record usage from a single request, including retry count.
    pub fn record(&self, usage: &Usage, retries: u32) {
        let mut stats = self.inner.lock().expect("usage tracker lock poisoned");
        stats.total_input_tokens += usage.input_tokens;
        stats.total_output_tokens += usage.output_tokens;
        stats.total_requests += 1;
        stats.total_retries += u64::from(retries);
    }

    /// Take a snapshot of the current accumulated statistics.
    pub fn snapshot(&self) -> UsageStats {
        self.inner.lock().expect("usage tracker lock poisoned").clone()
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        *self.inner.lock().expect("usage tracker lock poisoned") = UsageStats::default();
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_usage() {
        let tracker = UsageTracker::new();

        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            cached_input_tokens: 0,
        };
        tracker.record(&usage, 2);

        let snap = tracker.snapshot();
        assert_eq!(snap.total_input_tokens, 100);
        assert_eq!(snap.total_output_tokens, 50);
        assert_eq!(snap.total_requests, 1);
        assert_eq!(snap.total_retries, 2);
    }

    #[test]
    fn accumulates_across_requests() {
        let tracker = UsageTracker::new();

        let usage = Usage {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            cached_input_tokens: 0,
        };
        tracker.record(&usage, 0);
        tracker.record(&usage, 1);

        let snap = tracker.snapshot();
        assert_eq!(snap.total_input_tokens, 20);
        assert_eq!(snap.total_output_tokens, 10);
        assert_eq!(snap.total_requests, 2);
        assert_eq!(snap.total_retries, 1);
    }

    #[test]
    fn reset_clears_stats() {
        let tracker = UsageTracker::new();

        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            cached_input_tokens: 0,
        };
        tracker.record(&usage, 0);
        tracker.reset();

        let snap = tracker.snapshot();
        assert_eq!(snap.total_input_tokens, 0);
        assert_eq!(snap.total_requests, 0);
    }
}
