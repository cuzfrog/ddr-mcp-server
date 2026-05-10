use std::path::Path;

use crate::app::index::pipeline::IndexableDocument;
use crate::config::{IndexConfig};
use crate::domain::ChunkKind;
use crate::index::{
    read_bm25_index, IndexRepository, SourceIndexKind,
};
use crate::index::embedder::Embedder;
use crate::tests::fixtures::{
    make_temp_dir, FakeEmbedder,
};

fn create_minimal_file_index(persist_path: &Path) {
    let config = IndexConfig {
        embedding_model: "BGESmallENV15Q".to_string(),
        persist_path: persist_path.to_string_lossy().to_string(),
        chunk_size: 256,
        chunk_overlap: 32,
        max_size_mb: 512,
    };

    let repo = IndexRepository::new(persist_path, &config);

    let mut embedder = FakeEmbedder::new();
    let doc = IndexableDocument {
        source_path: "test.md".to_string(),
        source_revision: "abc".to_string(),
        title: "Test".to_string(),
        body: "Hello world".to_string(),
        modified_at: None,
        kind: ChunkKind::File,
        is_fresh: None,
    };

    let tok = embedder.token_counter();
    let pipeline = crate::app::index::pipeline::IndexingPipeline::new(&config, tok);
    let batch = pipeline.run(&[doc], &mut embedder, None, 1.2, 0.75).unwrap();
    let doc_count = crate::app::index::pipeline::unique_doc_count(&batch.metadata);
    repo.store(SourceIndexKind::File, &batch, embedder.dims(), doc_count, None)
        .unwrap();
}

fn create_file_index_without_bm25(persist_path: &Path) {
    create_minimal_file_index(persist_path);
    let bm25_dir = persist_path.join("file").join("bm25");
    let _ = std::fs::remove_dir_all(&bm25_dir);
}

fn create_git_index_without_bm25(persist_path: &Path) {
    let config = IndexConfig {
        embedding_model: "BGESmallENV15Q".to_string(),
        persist_path: persist_path.to_string_lossy().to_string(),
        chunk_size: 256,
        chunk_overlap: 32,
        max_size_mb: 512,
    };

    let repo = IndexRepository::new(persist_path, &config);

    let mut embedder = FakeEmbedder::new();
    let doc = IndexableDocument {
        source_path: "git-file.md".to_string(),
        source_revision: "def".to_string(),
        title: "Git Test".to_string(),
        body: "Git commit content for testing.".to_string(),
        modified_at: None,
        kind: ChunkKind::Git,
        is_fresh: None,
    };

    let tok = embedder.token_counter();
    let pipeline = crate::app::index::pipeline::IndexingPipeline::new(&config, tok);
    let batch = pipeline.run(&[doc], &mut embedder, None, 1.2, 0.75).unwrap();
    let doc_count = crate::app::index::pipeline::unique_doc_count(&batch.metadata);
    repo.store(SourceIndexKind::Git, &batch, embedder.dims(), doc_count, None)
        .unwrap();

    let bm25_dir = persist_path.join("git").join("bm25");
    let _ = std::fs::remove_dir_all(&bm25_dir);
}

#[test]
fn file_only_missing_bm25_rebuilds_on_load() {
    let persist = make_temp_dir("rebuild_file_bm25");
    create_file_index_without_bm25(&persist);

    assert!(
        !persist.join("file").join("bm25").join("header.json").exists(),
        "BM25 should be absent before load"
    );

    let config = IndexConfig {
        embedding_model: "BGESmallENV15Q".to_string(),
        persist_path: persist.to_string_lossy().to_string(),
        chunk_size: 256,
        chunk_overlap: 32,
        max_size_mb: 512,
    };
    let repo = IndexRepository::new(&persist, &config);
    let result = repo.load_merged(1.2, 0.75).unwrap();

    assert!(
        persist.join("file").join("bm25").join("header.json").exists(),
        "BM25 should be created after load"
    );

    assert!(
        result.notices.iter().any(|n| n.contains("Rebuilt BM25 index for file/")),
        "Expected rebuild notice for file/, got: {:?}",
        result.notices
    );

    let (_header, _embeddings) = read_bm25_index(&persist.join("file").join("bm25")).unwrap();
    assert!(!_embeddings.is_empty(), "BM25 embeddings should not be empty");

    let _ = std::fs::remove_dir_all(&persist);
}

#[test]
fn git_only_missing_bm25_rebuilds_on_load() {
    let persist = make_temp_dir("rebuild_git_bm25");
    create_git_index_without_bm25(&persist);

    assert!(
        !persist.join("git").join("bm25").join("header.json").exists(),
        "BM25 should be absent before load"
    );

    let config = IndexConfig {
        embedding_model: "BGESmallENV15Q".to_string(),
        persist_path: persist.to_string_lossy().to_string(),
        chunk_size: 256,
        chunk_overlap: 32,
        max_size_mb: 512,
    };
    let repo = IndexRepository::new(&persist, &config);
    let result = repo.load_merged(1.2, 0.75).unwrap();

    assert!(
        persist.join("git").join("bm25").join("header.json").exists(),
        "BM25 should be created after load"
    );

    assert!(
        result.notices.iter().any(|n| n.contains("Rebuilt BM25 index for git/")),
        "Expected rebuild notice for git/, got: {:?}",
        result.notices
    );

    let _ = std::fs::remove_dir_all(&persist);
}

#[test]
fn dual_source_one_side_missing_bm25() {
    let persist = make_temp_dir("rebuild_dual_bm25");
    create_minimal_file_index(&persist);
    create_git_index_without_bm25(&persist);

    let config = IndexConfig {
        embedding_model: "BGESmallENV15Q".to_string(),
        persist_path: persist.to_string_lossy().to_string(),
        chunk_size: 256,
        chunk_overlap: 32,
        max_size_mb: 512,
    };
    let repo = IndexRepository::new(&persist, &config);
    let result = repo.load_merged(1.2, 0.75).unwrap();

    assert!(
        persist.join("file").join("bm25").join("header.json").exists(),
        "File BM25 should still exist"
    );
    assert!(
        persist.join("git").join("bm25").join("header.json").exists(),
        "Git BM25 should have been created"
    );

    assert_eq!(result.notices.len(), 1, "Expected exactly 1 rebuild notice");
    assert!(
        result.notices[0].contains("Rebuilt BM25 index for git/"),
        "Expected git rebuild notice, got: {}",
        result.notices[0]
    );

    let _ = std::fs::remove_dir_all(&persist);
}

#[test]
fn idempotent_bm25_repair() {
    let persist = make_temp_dir("rebuild_idempotent");
    create_file_index_without_bm25(&persist);

    let config = IndexConfig {
        embedding_model: "BGESmallENV15Q".to_string(),
        persist_path: persist.to_string_lossy().to_string(),
        chunk_size: 256,
        chunk_overlap: 32,
        max_size_mb: 512,
    };
    let repo = IndexRepository::new(&persist, &config);

    let first = repo.load_merged(1.2, 0.75).unwrap();
    assert_eq!(first.notices.len(), 1, "First load should emit 1 notice");

    let second = repo.load_merged(1.2, 0.75).unwrap();
    assert!(
        second.notices.is_empty(),
        "Second load should NOT emit any notices, got: {:?}",
        second.notices
    );

    let _ = std::fs::remove_dir_all(&persist);
}
