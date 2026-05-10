use anyhow::Context;
use async_trait::async_trait;
use axum::Router;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

use crate::app::serve::service_builder::HybridServiceBuilder;
use crate::app::serve::ServeIndexAccess;
use crate::config::Config;
use crate::index::embedder_factory::EmbedderFactory;
use crate::mcp::DocentMcpServer;
use crate::mcp::SearchExecutor;
use crate::support::ui::Console;

#[async_trait]
pub trait Server: Send + Sync {
    async fn serve(
        &self,
        config: &Config,
        embedder_factory: &dyn EmbedderFactory,
        ui: &dyn Console,
    ) -> anyhow::Result<()>;
}

pub fn create_server(index_access: impl ServeIndexAccess + 'static) -> impl Server {
    TokioHttpServer { index_access: Box::new(index_access) }
}

struct TokioHttpServer {
    index_access: Box<dyn ServeIndexAccess>,
}

#[async_trait]
impl Server for TokioHttpServer {
    async fn serve(
        &self,
        config: &Config,
        embedder_factory: &dyn EmbedderFactory,
        ui: &dyn Console,
    ) -> anyhow::Result<()> {
        let router = prepare_router(&*self.index_access, embedder_factory, config, ui)?;

        let addr = format!("127.0.0.1:{}", config.server.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .context("Failed to bind TCP listener")?;
        let local_addr = listener
            .local_addr()
            .context("Failed to get local address")?;

        ui.info(&format!(
            "docent server listening on http://{} (open in browser for web UI)",
            local_addr,
        ));

        axum::serve(listener, router)
            .with_graceful_shutdown(super::bootstrap::shutdown_signal())
            .await
            .context("Server error")?;

        Ok(())
    }
}

fn prepare_router(
    index_access: &dyn ServeIndexAccess,
    embedder_factory: &dyn EmbedderFactory,
    config: &Config,
    ui: &dyn Console,
) -> anyhow::Result<Router> {
    let persist_path = config.persist_path_buf();

    if let Some(info) = index_access.check_size(&persist_path, config.index.max_size_mb)? {
        ui.warn(&format!(
            "The total index is {:.1} MB, which exceeds the configured limit of {} MB.",
            info.total_bytes as f64 / (1024.0 * 1024.0),
            config.index.max_size_mb
        ));
        if persist_path.join("file").exists() {
            ui.warn(&format!("  file/ subdirectory: {:.1} MB", info.file_bytes as f64 / (1024.0 * 1024.0)));
        }
        if persist_path.join("git").exists() {
            ui.warn(&format!("  git/ subdirectory:  {:.1} MB", info.git_bytes as f64 / (1024.0 * 1024.0)));
        }
        if !ui.confirm("Continue?")? {
            anyhow::bail!("Aborted by user.");
        }
    }

    let result = index_access
        .load_merged(&persist_path, &config.index, config.search.bm25.k1, config.search.bm25.b)
        .map_err(|e| anyhow::anyhow!("Failed to load merged index: {}", e))?;
    for notice in &result.notices {
        ui.info(notice);
    }
    let merged = result.merged;

    let builder = HybridServiceBuilder;
    let embedder = builder.build_embedder(embedder_factory, &config.index.embedding_model)?;
    let search_service = std::sync::Arc::new(builder.build(
        merged,
        embedder,
        &config.search,
    )?);

    let server = DocentMcpServer { search_executor: SearchExecutor::new(search_service) };
    let service: StreamableHttpService<DocentMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            {
                let server = server.clone();
                move || Ok(server.clone())
            },
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default(),
        );
    let router = crate::ui::router(service);

    Ok(router)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::app::serve::server::prepare_router;
    use crate::app::serve::ServeIndexAccess;
    use crate::config::{Config, IndexConfig};
    use crate::index::embedder::EmbeddingService;
    use crate::index::embedder_factory::EmbedderFactory;
    use crate::index::{
        IndexRepository, IndexSizeInfo, LoadMergedResult, MergedIndex, SourceIndexKind,
    };
    use crate::index::VectorStore;
    use crate::tests::fixtures::{
        make_temp_dir, FakeEmbedder, FakeEmbedderFactory, RecordingUi,
    };

    struct FakeServeIndexAccess {
        oversized: bool,
        load_error: bool,
    }

    impl FakeServeIndexAccess {
        fn new() -> Self {
            Self { oversized: false, load_error: false }
        }

        fn with_oversized(mut self) -> Self {
            self.oversized = true;
            self
        }

        fn with_load_error(mut self) -> Self {
            self.load_error = true;
            self
        }
    }

    impl ServeIndexAccess for FakeServeIndexAccess {
        fn check_size(
            &self,
            _persist_path: &Path,
            _max_size_mb: u64,
        ) -> anyhow::Result<Option<IndexSizeInfo>> {
            if self.oversized {
                Ok(Some(IndexSizeInfo {
                    total_bytes: 1024 * 1024 * 100,
                    file_bytes: 1024 * 1024 * 50,
                    git_bytes: 1024 * 1024 * 50,
                }))
            } else {
                Ok(None)
            }
        }

        fn load_merged(
            &self,
            _persist_path: &Path,
            _config: &IndexConfig,
            _k1: f32,
            _b: f32,
        ) -> anyhow::Result<LoadMergedResult> {
            if self.load_error {
                Err(anyhow::anyhow!("mock load error"))
            } else {
                Ok(LoadMergedResult {
                    merged: MergedIndex {
                        vectors: VectorStore::from_vec_vec(vec![]).unwrap(),
                        metadata: vec![],
                        bm25_embeddings: None,
                        bm25_header: None,
                        built_at: "2026-01-01T00:00:00Z".to_string(),
                    },
                    notices: vec![],
                })
            }
        }
    }

    struct FailingEmbedderFactory;

    impl EmbedderFactory for FailingEmbedderFactory {
        fn create(&self, _model: &str) -> anyhow::Result<Box<dyn EmbeddingService>> {
            Err(anyhow::anyhow!("mock embedder init error"))
        }
    }

    fn serve_config(persist_path: &Path) -> Config {
        Config {
            index: IndexConfig {
                embedding_model: "BGESmallENV15Q".to_string(),
                persist_path: persist_path.to_string_lossy().to_string(),
                chunk_size: 256,
                chunk_overlap: 32,
                max_size_mb: 512,
            },
            server: crate::config::ServerConfig {
                port: 9999,
                log_level: "info".to_string(),
            },
            search: crate::config::SearchConfig {
                ranking: crate::config::RankingConfig {
                    same_src_score_decay: 0.9,
                    file_hint_boost: 1.5,
                },
                fusion: crate::config::FusionConfig {
                    strategy: "rrf".to_string(),
                    rrf_k: 60.0,
                    semantic_weight: 0.7,
                },
                bm25: crate::config::Bm25Config {
                    k1: 1.2,
                    b: 0.75,
                },
            },
            git: None,
            file: None,
        }
    }

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
        let doc = crate::app::index::pipeline::IndexableDocument {
            source_path: "test.md".to_string(),
            source_revision: "abc".to_string(),
            title: "Test".to_string(),
            body: "Hello world".to_string(),
            modified_at: None,
            kind: crate::domain::ChunkKind::File,
            is_fresh: None,
        };

        let tok = embedder.token_counter();
        let pipeline = crate::app::index::pipeline::IndexingPipeline::new(&config, tok);
        let batch = pipeline.run(&[doc], &mut embedder, None, 1.2, 0.75).unwrap();
        let doc_count = crate::app::index::pipeline::unique_doc_count(&batch.metadata);
        repo.store(SourceIndexKind::File, &batch, embedder.dims(), doc_count, None)
            .unwrap();
    }

    #[test]
    fn oversized_index_aborts_when_not_confirmed() {
        let persist = make_temp_dir("serve_oversized_abort");
        let config = serve_config(&persist);
        let index_access = FakeServeIndexAccess::new().with_oversized();
        let ui = RecordingUi::never_confirm();
        let factory = FakeEmbedderFactory;

        let result = prepare_router(&index_access, &factory, &config, &ui);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Aborted"), "Expected abort error, got: {}", err);

        let _ = std::fs::remove_dir_all(&persist);
    }

    #[test]
    fn oversized_index_continues_when_confirmed() {
        let persist = make_temp_dir("serve_oversized_continue");
        create_minimal_file_index(&persist);
        let config = serve_config(&persist);
        let mut oversized_config = config.clone();
        oversized_config.index.max_size_mb = 1;
        let index_access = FakeServeIndexAccess::new().with_oversized();
        let ui = RecordingUi::always_confirm();
        let factory = FakeEmbedderFactory;

        let result = prepare_router(&index_access, &factory, &oversized_config, &ui);
        assert!(result.is_ok(), "Expected success, got: {:?}", result.err());

        let _ = std::fs::remove_dir_all(&persist);
    }

    #[test]
    fn merged_index_loading_error_propagates() {
        let persist = make_temp_dir("serve_merge_error");
        let config = serve_config(&persist);
        let index_access = FakeServeIndexAccess::new().with_load_error();
        let ui = RecordingUi::always_confirm();
        let factory = FakeEmbedderFactory;

        let result = prepare_router(&index_access, &factory, &config, &ui);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(
            display.contains("Failed to load merged index"),
            "Expected context message about loading, got: {}",
            display
        );
        let cause_found = err.chain().any(|e| e.to_string().contains("mock load error"));
        assert!(
            cause_found,
            "Expected mock load error in chain, got: {:#}",
            err
        );

        let _ = std::fs::remove_dir_all(&persist);
    }

    #[test]
    fn embedder_init_error_propagates() {
        let persist = make_temp_dir("serve_embedder_error");
        let config = serve_config(&persist);
        let index_access = FakeServeIndexAccess::new();
        let ui = RecordingUi::always_confirm();
        let factory = FailingEmbedderFactory;

        let result = prepare_router(&index_access, &factory, &config, &ui);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(
            display.contains("Failed to initialize embedding model"),
            "Expected context message about embedding model, got: {}",
            display
        );
        let cause_found = err.chain().any(|e| e.to_string().contains("mock embedder init error"));
        assert!(
            cause_found,
            "Expected mock embedder init error in chain, got: {:#}",
            err
        );

        let _ = std::fs::remove_dir_all(&persist);
    }

    #[test]
    fn bootstrap_succeeds_with_fake_dependencies() {
        let persist = make_temp_dir("serve_bootstrap");
        create_minimal_file_index(&persist);
        let config = serve_config(&persist);
        let index_access = FakeServeIndexAccess::new();
        let ui = RecordingUi::always_confirm();
        let factory = FakeEmbedderFactory;

        let result = prepare_router(&index_access, &factory, &config, &ui);
        assert!(result.is_ok(), "Expected success, got: {:?}", result.err());

        let _ = std::fs::remove_dir_all(&persist);
    }
}
