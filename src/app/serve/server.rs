use anyhow::Context;
use async_trait::async_trait;
use axum::Router;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

use crate::app::serve::bootstrap::PreparedServe;
use crate::app::serve::service_builder::HybridServiceBuilder;
use crate::app::serve::ServeIndexAccess;
use crate::config::Config;
use crate::index::embedder::EmbedderFactory;
use crate::mcp::DocentMcpServer;
use crate::mcp::SearchExecutor;
use crate::support::ui::Console;

#[async_trait]
pub trait Server: Send + Sync {
    async fn serve(&self, router: Router, port: u16, ui: &dyn Console) -> anyhow::Result<()>;
}

pub struct TokioHttpServer;

#[async_trait]
impl Server for TokioHttpServer {
    async fn serve(&self, router: Router, port: u16, ui: &dyn Console) -> anyhow::Result<()> {
        let addr = format!("127.0.0.1:{}", port);
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

pub(crate) fn prepare_serve(
    index_access: &dyn ServeIndexAccess,
    embedder_factory: &dyn EmbedderFactory,
    config: &Config,
    ui: &dyn Console,
) -> anyhow::Result<PreparedServe> {
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

    Ok(PreparedServe { router })
}
