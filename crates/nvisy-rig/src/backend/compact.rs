//! Context window management for LLM token limits.

/// Manages token budget estimation, splitting, and truncation.
pub struct ContextWindow {
    /// Maximum tokens the model supports.
    max_tokens: usize,
    /// Tokens reserved for the output/completion.
    reserve_output: usize,
}

impl ContextWindow {
    /// Create a new context window with the given limits.
    pub fn new(max_tokens: usize, reserve_output: usize) -> Self {
        Self {
            max_tokens,
            reserve_output,
        }
    }

    /// Estimate the number of tokens in a string (~4 chars per token).
    pub fn estimate_tokens(text: &str) -> usize {
        // Rough heuristic: ~4 characters per token for English text.
        (text.len() + 3) / 4
    }

    /// Available input token budget (max minus reserved output).
    fn input_budget(&self) -> usize {
        self.max_tokens.saturating_sub(self.reserve_output)
    }

    /// Check if the text fits within the available input budget.
    pub fn fits(&self, text: &str) -> bool {
        Self::estimate_tokens(text) <= self.input_budget()
    }

    /// Split text into chunks that each fit within the input budget.
    ///
    /// Splitting respects sentence boundaries (`. ` and `\n`) where possible.
    pub fn split_to_fit<'a>(&self, text: &'a str) -> Vec<&'a str> {
        if self.fits(text) {
            return vec![text];
        }

        let budget = self.input_budget();
        // Approximate char budget from token budget.
        let char_budget = budget * 4;

        let mut chunks = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            if Self::estimate_tokens(remaining) <= budget {
                chunks.push(remaining);
                break;
            }

            // Take up to char_budget characters, then find a sentence boundary.
            let take = remaining.len().min(char_budget);
            let candidate = &remaining[..take];

            // Try to split at the last sentence boundary within the candidate.
            let split_pos = find_last_boundary(candidate).unwrap_or(take);

            let (chunk, rest) = remaining.split_at(split_pos);
            if chunk.is_empty() {
                // No boundary found within budget; force-split at char_budget.
                let forced = remaining.len().min(char_budget);
                let (chunk, rest) = remaining.split_at(forced);
                chunks.push(chunk);
                remaining = rest;
            } else {
                chunks.push(chunk);
                remaining = rest.trim_start_matches(['\n', ' ']);
            }
        }

        chunks
    }

    /// Truncate text to fit, keeping the end (most recent context).
    pub fn truncate_to_fit<'a>(&self, text: &'a str) -> &'a str {
        if self.fits(text) {
            return text;
        }

        let budget = self.input_budget();
        let char_budget = budget * 4;

        if text.len() <= char_budget {
            return text;
        }

        let start = text.len() - char_budget;
        // Try to start at a boundary to avoid splitting mid-sentence.
        let adjusted = text[start..]
            .find(['\n', '.'])
            .map(|pos| start + pos + 1)
            .unwrap_or(start);

        &text[adjusted.min(text.len())..]
    }
}

/// Find the last sentence boundary (`. ` or `\n`) in the text.
fn find_last_boundary(text: &str) -> Option<usize> {
    let last_newline = text.rfind('\n');
    let last_period = text.rfind(". ").map(|p| p + 2);

    match (last_newline, last_period) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_basic() {
        assert_eq!(ContextWindow::estimate_tokens(""), 0);
        assert_eq!(ContextWindow::estimate_tokens("abcd"), 1);
        assert_eq!(ContextWindow::estimate_tokens("abcdefgh"), 2);
    }

    #[test]
    fn fits_within_budget() {
        let cw = ContextWindow::new(100, 20);
        // Budget = 80 tokens = ~320 chars
        let short = "a".repeat(300);
        assert!(cw.fits(&short));

        let long = "a".repeat(400);
        assert!(!cw.fits(&long));
    }

    #[test]
    fn split_short_text() {
        let cw = ContextWindow::new(100, 20);
        let text = "hello world";
        let chunks = cw.split_to_fit(text);
        assert_eq!(chunks, vec!["hello world"]);
    }

    #[test]
    fn truncate_keeps_end() {
        let cw = ContextWindow::new(10, 2);
        // Budget = 8 tokens = ~32 chars
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence.";
        let truncated = cw.truncate_to_fit(text);
        // Should keep the tail end
        assert!(truncated.len() <= 32 + 10); // some slack for boundary adjustment
        assert!(text.ends_with(truncated) || truncated.contains("sentence"));
    }
}
