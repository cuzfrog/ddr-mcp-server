mod engine;

pub use crate::domain::{IndexableDocument, IndexedBatch};
pub use engine::{IndexingProcessor, create_processor};
