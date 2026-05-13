// ---------------------------------------------------------------------------
// TokenCounter trait — swappable tokenizer abstraction
// ---------------------------------------------------------------------------

pub trait TokenCounter: Send + Sync {
    /// Encode `text` and return (total_token_count, Vec<(byte_start, byte_end)>).
    /// `byte_start` and `byte_end` are UTF-8 byte offsets into the original `text`.
    fn encode_with_offsets(&self, text: &str) -> (usize, Vec<(usize, usize)>);
}

// ---------------------------------------------------------------------------
// WhitespaceTokenCounter — mock for unit tests
// ---------------------------------------------------------------------------

pub(crate) struct WhitespaceTokenCounter;

impl TokenCounter for WhitespaceTokenCounter {
    fn encode_with_offsets(&self, text: &str) -> (usize, Vec<(usize, usize)>) {
        let mut offsets = Vec::new();
        let mut byte_pos = 0;

        let trimmed = text;
        for word in trimmed.split_whitespace() {
            if let Some(pos) = trimmed[byte_pos..].find(word) {
                let start = byte_pos + pos;
                let end = start + word.len();
                offsets.push((start, end));
                byte_pos = end;
            }
        }

        (offsets.len(), offsets)
    }
}

// ---------------------------------------------------------------------------
// HuggingFaceTokenCounter — real tokenizer using the embedding model's tokenizer
// ---------------------------------------------------------------------------

struct HuggingFaceTokenCounter {
    tokenizer: tokenizers::Tokenizer,
}

pub fn create_token_counter(tokenizer: tokenizers::Tokenizer) -> Box<dyn TokenCounter> {
    Box::new(HuggingFaceTokenCounter { tokenizer })
}

impl TokenCounter for HuggingFaceTokenCounter {
    fn encode_with_offsets(&self, text: &str) -> (usize, Vec<(usize, usize)>) {
        match self.tokenizer.encode(text, false) {
            Ok(encoding) => {
                let offsets = encoding.get_offsets().to_vec();
                (offsets.len(), offsets)
            }
            Err(e) => {
                eprintln!(
                    "WARNING: tokenizer.encode failed: {e}. Falling back to whitespace offsets."
                );
                let counter = WhitespaceTokenCounter;
                counter.encode_with_offsets(text)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: count tokens using `encode_with_offsets`
    fn count_tokens(counter: &dyn TokenCounter, text: &str) -> usize {
        counter.encode_with_offsets(text).0
    }

    #[test]
    fn test_whitespace_counter_basics() {
        let counter = WhitespaceTokenCounter;
        assert_eq!(count_tokens(&counter, ""), 0);
        assert_eq!(count_tokens(&counter, "   "), 0);
        assert_eq!(count_tokens(&counter, "hello"), 1);
        assert_eq!(count_tokens(&counter, "hello world"), 2);

        let (count, offsets) = counter.encode_with_offsets("hello world");
        assert_eq!(count, 2);
        assert_eq!(offsets, vec![(0, 5), (6, 11)]);
    }
}
