mod schema;
mod bm25_schema;
mod bm25_storage;
mod storage;
mod repository;
mod sub_index;

#[derive(Clone, Copy)]
pub enum SourceIndexKind {
    File,
    Git,
}

impl SourceIndexKind {
    pub(crate) fn subdir(&self) -> &str {
        match self {
            SourceIndexKind::File => "file",
            SourceIndexKind::Git => "git",
        }
    }
}

#[cfg(test)]
pub(crate) use bm25_storage::read_bm25_index;
pub(crate) use repository::{IndexRepository, IndexSizeInfo, LoadMergedResult, MergedIndex};
pub(crate) use schema::VectorStore;
#[cfg(test)]
pub(crate) use schema::{IndexHeader, SCHEMA_VERSION};
