mod types;
mod engine;

pub use engine::{IndexingProcessor, create_processor};
pub use types::{IndexableDocument, IndexedBatch};
pub(crate) use types::unique_doc_count;

#[cfg(test)]
pub(crate) use engine::create_test_processor;
