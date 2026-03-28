//! Cumulative token-usage tracking across LLM requests.

use std::sync::Mutex;

use rig::completion::Usage;

/// Thread-safe accumulator for LLM token usage.
///
/// Each agent owns one tracker; callers snapshot it to inspect costs.
pub struct UsageTracker {
    inner: Mutex<UsageStats>,
}

/// Point-in-time snapshot of accumulated usage counters.
#[derive(Debug, Default, Clone)]
pub struct UsageStats {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_requests: u64,
    pub total_retries: u64,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(UsageStats::default()),
        }
    }

    /// Record a single LLM request's token usage and retry count.
    pub fn record(&self, usage: &Usage, retries: u32) {
        let mut stats = self.inner.lock().expect("usage tracker lock poisoned");
        stats.total_input_tokens += usage.input_tokens;
        stats.total_output_tokens += usage.output_tokens;
        stats.total_requests += 1;
        stats.total_retries += u64::from(retries);
    }

    /// Snapshot the current counters without resetting them.
    pub fn snapshot(&self) -> UsageStats {
        self.inner
            .lock()
            .expect("usage tracker lock poisoned")
            .clone()
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
