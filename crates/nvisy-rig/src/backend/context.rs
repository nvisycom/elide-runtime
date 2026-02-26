//! Token budget estimation, text splitting, and truncation.
//!
//! [`ContextWindow`] provides a simple heuristic (~4 chars/token) to decide
//! whether text fits within a model's input budget and, when it doesn't,
//! to split or truncate it at sentence boundaries while staying UTF-8 safe.

/// Token budget manager for a single model context window.
///
/// All arithmetic is based on a rough **4 characters ≈ 1 token** heuristic.
/// This is intentionally conservative: over-splitting is harmless while
/// exceeding the real limit causes provider errors.
#[derive(Debug, Clone)]
pub struct ContextWindow {
    /// Maximum tokens the model supports.
    max_tokens: usize,
    /// Tokens reserved for the output/completion.
    reserve_output: usize,
}

impl ContextWindow {
    pub fn new(max_tokens: usize, reserve_output: usize) -> Self {
        Self {
            max_tokens,
            reserve_output,
        }
    }

    /// Rough token count (~4 chars per token for English text).
    pub fn estimate_tokens(text: &str) -> usize {
        text.len().div_ceil(4)
    }

    /// Input token budget (`max_tokens − reserve_output`).
    pub(crate) fn input_budget(&self) -> usize {
        self.max_tokens.saturating_sub(self.reserve_output)
    }

    /// Whether `text` fits within the input budget.
    pub fn fits(&self, text: &str) -> bool {
        Self::estimate_tokens(text) <= self.input_budget()
    }

    /// Split text into chunks that each fit within the input budget.
    ///
    /// Prefers sentence boundaries (`. ` and `\n`) and is safe for
    /// multi-byte UTF-8.
    pub fn split_to_fit<'a>(&self, text: &'a str) -> Vec<&'a str> {
        if self.fits(text) {
            return vec![text];
        }

        let budget = self.input_budget();
        let char_budget = budget * 4;

        let mut chunks = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            if Self::estimate_tokens(remaining) <= budget {
                chunks.push(remaining);
                break;
            }

            let take = snap_to_boundary(remaining, remaining.len().min(char_budget));
            let candidate = &remaining[..take];
            let split_pos = find_last_boundary(candidate).unwrap_or(take);

            let (chunk, rest) = remaining.split_at(split_pos);
            if chunk.is_empty() {
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

    /// Truncate text to fit, keeping the **tail** (most recent context).
    ///
    /// Safe for multi-byte UTF-8.
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
        let adjusted = text[start..]
            .find(['\n', '.'])
            .map(|pos| start + pos + 1)
            .unwrap_or(start);

        let adjusted = snap_to_boundary(text, adjusted.min(text.len()));
        &text[adjusted..]
    }
}

/// Snap a byte position to the nearest valid UTF-8 char boundary (walks backward).
fn snap_to_boundary(text: &str, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while p > 0 && !text.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Last sentence boundary (`. ` or `\n`) in `text`.
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
    fn fits_within_budget() {
        let cw = ContextWindow::new(100, 20);
        assert!(cw.fits(&"a".repeat(300)));   // ~75 tokens, budget 80
        assert!(!cw.fits(&"a".repeat(400)));  // ~100 tokens, budget 80
    }

    #[test]
    fn truncate_keeps_end() {
        let cw = ContextWindow::new(10, 2); // budget = 8 tokens ≈ 32 chars
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence.";
        let truncated = cw.truncate_to_fit(text);
        assert!(truncated.len() <= 42); // 32 + slack for boundary
        assert!(text.ends_with(truncated) || truncated.contains("sentence"));
    }

    #[test]
    fn snap_to_boundary_multibyte() {
        let text = "a🔥b"; // byte 0: 'a', bytes 1–4: '🔥', byte 5: 'b'
        assert_eq!(snap_to_boundary(text, 1), 1);
        assert_eq!(snap_to_boundary(text, 2), 1); // mid-emoji → snap back
        assert_eq!(snap_to_boundary(text, 3), 1);
        assert_eq!(snap_to_boundary(text, 4), 1);
        assert_eq!(snap_to_boundary(text, 5), 5);
    }

    #[test]
    fn split_to_fit_emoji() {
        let cw = ContextWindow::new(4, 2); // budget = 2 tokens ≈ 8 bytes
        let text = "🔥🔥🔥🔥"; // 16 bytes
        let chunks = cw.split_to_fit(text);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(!chunk.is_empty());
        }
    }

    #[test]
    fn split_to_fit_cjk() {
        let cw = ContextWindow::new(4, 2); // budget ≈ 8 bytes
        let text = "你好世界测试文字"; // 24 bytes (3 bytes × 8 chars)
        let chunks = cw.split_to_fit(text);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(!chunk.is_empty());
        }
    }

    #[test]
    fn truncate_to_fit_emoji() {
        let cw = ContextWindow::new(4, 2); // budget ≈ 8 bytes
        let text = "🔥🔥🔥🔥"; // 16 bytes
        let truncated = cw.truncate_to_fit(text);
        assert!(!truncated.is_empty());
        assert!(text.ends_with(truncated));
    }
}
