//! Context window management for LLM token limits.

use rig::completion::CompletionModel;

use nvisy_core::Error;

use super::agent::BaseAgent;

/// Manages token budget estimation, splitting, and truncation.
#[derive(Debug, Clone)]
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
    /// Splitting respects sentence boundaries (`. ` and `\n`) where possible
    /// and is safe for multi-byte UTF-8 input.
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

            // Take up to char_budget bytes, snapped to a char boundary.
            let take = snap_to_boundary(remaining, remaining.len().min(char_budget));
            let candidate = &remaining[..take];

            // Try to split at the last sentence boundary within the candidate.
            let split_pos = find_last_boundary(candidate).unwrap_or(take);

            let (chunk, rest) = remaining.split_at(split_pos);
            if chunk.is_empty() {
                // No boundary found within budget; force-split at char_budget.
                let forced = snap_to_boundary(remaining, remaining.len().min(char_budget));
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

    /// Summarize text via LLM to fit within the input token budget.
    ///
    /// If the text already fits, returns it unchanged. Otherwise sends a
    /// summarization prompt to the given agent and returns the condensed
    /// version.
    pub(crate) async fn compact<M: CompletionModel>(
        &self,
        text: &str,
        agent: &BaseAgent<M>,
    ) -> Result<String, Error> {
        if self.fits(text) {
            return Ok(text.to_owned());
        }

        let budget = self.input_budget();
        let prompt = format!(
            "Summarize the following text to fit within {budget} tokens. \
             Preserve all key entities, names, numbers, dates, and facts. \
             Remove redundancy and filler. Return ONLY the condensed text, \
             no preamble.\n\n{text}"
        );

        agent.prompt_text(&prompt).await
    }

    /// Truncate text to fit, keeping the end (most recent context).
    ///
    /// Safe for multi-byte UTF-8 input.
    pub fn truncate_to_fit<'a>(&self, text: &'a str) -> &'a str {
        if self.fits(text) {
            return text;
        }

        let budget = self.input_budget();
        let char_budget = budget * 4;

        if text.len() <= char_budget {
            return text;
        }

        let start = snap_to_boundary(text, text.len() - char_budget);
        // Try to start at a boundary to avoid splitting mid-sentence.
        let adjusted = text[start..]
            .find(['\n', '.'])
            .map(|pos| start + pos + 1)
            .unwrap_or(start);

        let adjusted = snap_to_boundary(text, adjusted.min(text.len()));
        &text[adjusted..]
    }
}

/// Snap a byte position to the nearest valid UTF-8 char boundary,
/// walking backward if necessary.
fn snap_to_boundary(text: &str, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while p > 0 && !text.is_char_boundary(p) {
        p -= 1;
    }
    p
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

    #[test]
    fn snap_to_boundary_ascii() {
        let text = "hello";
        assert_eq!(super::snap_to_boundary(text, 3), 3);
        assert_eq!(super::snap_to_boundary(text, 10), 5); // clamps to len
    }

    #[test]
    fn snap_to_boundary_multibyte() {
        // '🔥' is 4 bytes
        let text = "a🔥b";
        // byte 0: 'a', bytes 1-4: '🔥', byte 5: 'b'
        assert_eq!(super::snap_to_boundary(text, 1), 1); // valid
        assert_eq!(super::snap_to_boundary(text, 2), 1); // mid-emoji → snap back
        assert_eq!(super::snap_to_boundary(text, 3), 1); // mid-emoji → snap back
        assert_eq!(super::snap_to_boundary(text, 4), 1); // mid-emoji → snap back
        assert_eq!(super::snap_to_boundary(text, 5), 5); // valid (after emoji)
    }

    #[test]
    fn split_to_fit_emoji() {
        // Budget: 2 tokens = ~8 bytes. Each emoji is 4 bytes.
        let cw = ContextWindow::new(4, 2);
        let text = "🔥🔥🔥🔥"; // 16 bytes total
        let chunks = cw.split_to_fit(text);
        // Should not panic and every chunk must be valid UTF-8
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(!chunk.is_empty());
        }
    }

    #[test]
    fn split_to_fit_cjk() {
        // CJK chars are 3 bytes each
        let cw = ContextWindow::new(4, 2);
        // Budget: 2 tokens = ~8 bytes → fits 2 CJK chars (6 bytes)
        let text = "你好世界测试文字"; // 8 chars × 3 bytes = 24 bytes
        let chunks = cw.split_to_fit(text);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(!chunk.is_empty());
        }
    }

    #[test]
    fn truncate_to_fit_emoji() {
        let cw = ContextWindow::new(4, 2);
        // Budget: 2 tokens = ~8 bytes
        let text = "🔥🔥🔥🔥"; // 16 bytes
        let truncated = cw.truncate_to_fit(text);
        // Should not panic, should be valid UTF-8, and should be the tail
        assert!(!truncated.is_empty());
        assert!(text.ends_with(truncated));
    }

    #[test]
    fn compact_returns_unchanged_when_fits() {
        // compact requires async + a real model, so we only test the
        // early-return path via `fits` logic.  The "already fits" branch
        // returns `Ok(text.to_owned())` synchronously — verify the
        // prerequisite here.
        let cw = ContextWindow::new(100, 20);
        let short = "a".repeat(300); // ~75 tokens, budget is 80
        assert!(cw.fits(&short));
    }
}
