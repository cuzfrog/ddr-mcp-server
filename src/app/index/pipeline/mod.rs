mod types;
mod engine;

pub use engine::{IndexingProcessor, create_processor};
pub use types::{IndexableDocument, IndexedBatch};

#[cfg(test)]
pub(crate) use engine::create_test_processor;
