// ---------------------------------------------------------------------------
// TokenCounter trait — swappable tokenizer abstraction
// ---------------------------------------------------------------------------

#[cfg_attr(test, mockall::automock)]
pub trait TokenCounter: Send + Sync {
    /// Encode `text` and return (total_token_count, Vec<(byte_start, byte_end)>).
    /// `byte_start` and `byte_end` are UTF-8 byte offsets into the original `text`.
    fn encode_with_offsets(&self, text: &str) -> (usize, Vec<(usize, usize)>);
}

// ---------------------------------------------------------------------------
// HuggingFaceTokenCounter — real tokenizer using the embedding model's tokenizer
// ---------------------------------------------------------------------------

struct HuggingFaceTokenCounter {
    tokenizer: Box<dyn crate::models::Tokenizer>,
}

pub fn create_token_counter(tokenizer: Box<dyn crate::models::Tokenizer>) -> Box<dyn TokenCounter> {
    Box::new(HuggingFaceTokenCounter { tokenizer })
}

impl TokenCounter for HuggingFaceTokenCounter {
    fn encode_with_offsets(&self, text: &str) -> (usize, Vec<(usize, usize)>) {
        self.tokenizer.encode_with_offsets(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: count tokens using `encode_with_offsets`
    fn count_tokens(counter: &dyn TokenCounter, text: &str) -> usize {
        counter.encode_with_offsets(text).0
    }

    fn make_mock_counter() -> MockTokenCounter {
        let mut mock = MockTokenCounter::new();
        mock.expect_encode_with_offsets()
            .returning(|text: &str| {
                let mut offsets = Vec::new();
                let mut pos = 0;
                for word in text.split_whitespace() {
                    let start = pos + text[pos..].find(word).unwrap();
                    let end = start + word.len();
                    offsets.push((start, end));
                    pos = end;
                }
                (offsets.len(), offsets)
            });
        mock
    }

    #[test]
    fn test_whitespace_counter_basics() {
        let counter = make_mock_counter();
        assert_eq!(count_tokens(&counter, ""), 0);
        assert_eq!(count_tokens(&counter, "   "), 0);
        assert_eq!(count_tokens(&counter, "hello"), 1);
        assert_eq!(count_tokens(&counter, "hello world"), 2);

        let (count, offsets) = counter.encode_with_offsets("hello world");
        assert_eq!(count, 2);
        assert_eq!(offsets, vec![(0, 5), (6, 11)]);
    }
}
