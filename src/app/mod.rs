use std::path::PathBuf;

use crate::app::serve::server::{Server, create_server};
use crate::config::{defaults::DEFAULT_TEMPLATE, Config};
use crate::index::embedder::list_supported_models;
use crate::index::embedder_factory::{EmbedderFactory, create_embedder_factory};
use crate::support::ui::{Console, create_console};

pub(crate) mod index;
pub(crate) mod init;
pub mod serve;

pub struct Application {
    ui: Box<dyn Console>,
    embedder_factory: Box<dyn EmbedderFactory>,
    server: Box<dyn Server>,
}

impl Default for Application {
    fn default() -> Self {
        Self::new(
            Box::new(create_console(false)),
            Box::new(create_embedder_factory()),
            Box::new(create_server(crate::app::serve::create_serve_index_access())),
        )
    }
}

impl Application {
    pub fn new(
        ui: Box<dyn Console>,
        embedder_factory: Box<dyn EmbedderFactory>,
        server: Box<dyn Server>,
    ) -> Self {
        Self { ui, embedder_factory, server }
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
        self.server.serve(config, &*self.embedder_factory, &*self.ui).await
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
        _verbose: bool,
    ) -> anyhow::Result<()> {
        let request = index::file::FileIndexRequest {
            input_root,
            rebuild,
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
    use crate::app::serve::create_serve_index_access;
    use crate::tests::fixtures::make_temp_dir;

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
            Box::new(create_console(false)),
            Box::new(create_embedder_factory()),
            Box::new(create_server(create_serve_index_access())),
        );

        app.run_index(&config, Some(dir.clone()), false, false).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
