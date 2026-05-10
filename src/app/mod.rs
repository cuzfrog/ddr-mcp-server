use std::path::PathBuf;

use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

use crate::app::serve::bootstrap::PreparedServe;
use crate::app::serve::server::{Server, TokioHttpServer};
use crate::app::serve::service_builder::HybridServiceBuilder;
use crate::app::serve::{RealServeIndexAccess, ServeIndexAccess};
use crate::config::{defaults::DEFAULT_TEMPLATE, Config};
use crate::embedder::{list_supported_models, EmbedderFactory, RealEmbedderFactory};
use crate::interfaces::mcp::DocentMcpServer;
use crate::interfaces::search_tool::SearchExecutor;
use crate::support::ui::{ConsoleUi, WorkflowUi};

pub(crate) mod index;
pub(crate) mod init;
pub mod serve;

pub struct Application {
    ui: Box<dyn WorkflowUi>,
    embedder_factory: Box<dyn EmbedderFactory>,
    index_access: Box<dyn ServeIndexAccess>,
    server: Box<dyn Server>,
}

impl Default for Application {
    fn default() -> Self {
        Self::new(
            Box::new(ConsoleUi),
            Box::new(RealEmbedderFactory),
            Box::new(RealServeIndexAccess),
            Box::new(TokioHttpServer),
        )
    }
}

impl Application {
    pub fn new(
        ui: Box<dyn WorkflowUi>,
        embedder_factory: Box<dyn EmbedderFactory>,
        index_access: Box<dyn ServeIndexAccess>,
        server: Box<dyn Server>,
    ) -> Self {
        Self { ui, embedder_factory, index_access, server }
    }

    pub fn run_init(&self) -> anyhow::Result<()> {
        let target = PathBuf::from("./docent.toml");
        if target.exists() {
            let existing = std::fs::read_to_string(&target)?;
            let merged = init::merge_toml(DEFAULT_TEMPLATE, &existing)?;
            std::fs::write(&target, &merged)?;
            self.ui.info(&format!("Merged new config fields into {}", target.display()));
        } else {
            std::fs::write(&target, DEFAULT_TEMPLATE)?;
            self.ui.info(&format!("Generated {}", target.display()));
        }
        Ok(())
    }

    pub fn list_models(&self) {
        for (name, dim) in list_supported_models() {
            self.ui.info(&format!("{} (dim: {})", name, dim));
        }
    }

    pub fn run_index(
        &self,
        config: &Config,
        input_path: Option<PathBuf>,
        rebuild: bool,
        verbose: bool,
    ) -> anyhow::Result<()> {
        let dir = input_path.unwrap_or_else(|| PathBuf::from("."));
        let dir = dir.canonicalize()?;

        let file_enabled = config.file.as_ref().map(|f| f.enabled).unwrap_or(true);
        if file_enabled {
            self.run_file_index_workflow(config, dir.clone(), rebuild, verbose)?;
        }

        let git_enabled = config.git.as_ref().map(|g| g.enabled).unwrap_or(false);
        if git_enabled {
            self.run_git_index_workflow(config, dir, rebuild, verbose)?;
        }

        Ok(())
    }

    pub async fn run_serve(&self, config: &Config) -> anyhow::Result<()> {
        self.ui.info(&format!(
            "Serving index at {}",
            config.persist_path_buf().display(),
        ));

        let prepared = self.prepare_serve(config)?;
        self.server.serve(prepared.router, config.server.port, &*self.ui).await
    }

    pub(crate) fn prepare_serve(&self, config: &Config) -> anyhow::Result<PreparedServe> {
        let persist_path = config.persist_path_buf();

        if let Some(info) = self.index_access.check_size(&persist_path, config.index.max_size_mb)? {
            self.ui.warn(&format!(
                "The total index is {:.1} MB, which exceeds the configured limit of {} MB.",
                info.total_bytes as f64 / (1024.0 * 1024.0),
                config.index.max_size_mb
            ));
            if persist_path.join("file").exists() {
                self.ui.warn(&format!("  file/ subdirectory: {:.1} MB", info.file_bytes as f64 / (1024.0 * 1024.0)));
            }
            if persist_path.join("git").exists() {
                self.ui.warn(&format!("  git/ subdirectory:  {:.1} MB", info.git_bytes as f64 / (1024.0 * 1024.0)));
            }
            if !self.ui.confirm("Continue?")? {
                anyhow::bail!("Aborted by user.");
            }
        }

        let result = self.index_access
            .load_merged(&persist_path, &config.index, config.search.bm25.k1, config.search.bm25.b)
            .map_err(|e| anyhow::anyhow!("Failed to load merged index: {}", e))?;
        for notice in &result.notices {
            self.ui.info(notice);
        }
        let merged = result.merged;

        let builder = HybridServiceBuilder;
        let embedder = builder.build_embedder(&*self.embedder_factory, &config.index.embedding_model)?;
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

    fn emit_outcome(&self, outcome: Vec<(&'static str, String)>) {
        for (level, msg) in outcome {
            match level {
                "warn" => self.ui.warn(&msg),
                _ => self.ui.info(&msg),
            }
        }
    }

    fn run_file_index_workflow(
        &self,
        config: &Config,
        input_root: PathBuf,
        rebuild: bool,
        verbose: bool,
    ) -> anyhow::Result<()> {
        let request = index::file::FileIndexRequest {
            input_root,
            rebuild,
            verbose,
        };
        let workflow = index::file::FileIndexWorkflow::new(config, &*self.ui, &*self.embedder_factory);
        let outcome = workflow.run(request)?;
        self.emit_outcome(outcome.format_for_ui());
        Ok(())
    }

    fn run_git_index_workflow(
        &self,
        config: &Config,
        repo_path: PathBuf,
        rebuild: bool,
        verbose: bool,
    ) -> anyhow::Result<()> {
        let request = index::git::GitIndexRequest {
            repo_path,
            rebuild,
            verbose,
        };
        let workflow = index::git::GitIndexWorkflow::new(config, &*self.ui, &*self.embedder_factory);
        let outcome = workflow.run(request)?;
        self.emit_outcome(outcome.format_for_ui());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::serve::RealServeIndexAccess;
    use crate::tests::fixtures::{FakeEmbedderFactory, RecordingUi, make_temp_dir};

    #[test]
    fn format_supported_models_returns_expected_strings() {
        let models = [
            ("model-a".to_string(), 384),
            ("model-b".to_string(), 768),
        ];
        let formatted: Vec<String> = models.iter()
            .map(|(name, dim)| format!("{} (dim: {})", name, dim))
            .collect();
        assert_eq!(formatted, vec!["model-a (dim: 384)", "model-b (dim: 768)"]);
    }

    #[test]
    fn format_supported_models_empty() {
        let formatted: Vec<String> = vec![];
        assert!(formatted.is_empty());
    }

    #[test]
    fn run_index_skips_both_when_file_disabled_and_git_absent() {
        let dir = make_temp_dir("app_index_both_skip");

        let config = Config {
            index: crate::config::IndexConfig {
                embedding_model: "BGESmallENV15Q".to_string(),
                ..Default::default()
            },
            file: Some(crate::config::FileConfig {
                enabled: false,
                glob_patterns: vec![],
                file_size_limit_mb: 0,
            }),
            git: None,
            ..Default::default()
        };

        let app = Application::new(
            Box::new(RecordingUi::always_confirm()),
            Box::new(FakeEmbedderFactory),
            Box::new(RealServeIndexAccess),
            Box::new(TokioHttpServer),
        );

        app.run_index(&config, Some(dir.clone()), false, false).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
