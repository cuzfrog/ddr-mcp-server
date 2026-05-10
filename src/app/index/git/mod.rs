use std::path::PathBuf;

use crate::config::Config;
use crate::index::embedder::dims_for_model;

use crate::index::{IndexRepository, SourceIndexKind};
use crate::support::ui::Console;

pub(crate) mod rebuild;
pub(crate) mod incremental;
pub(crate) mod size_check;

mod estimate;
pub(crate) mod extract;
mod freshness;
pub(crate) mod history;
mod merge;

pub(crate) use estimate::{estimate_commit_count, estimate_git_index_size};
pub(crate) use extract::prepare_git_documents;
pub(crate) use freshness::compute_freshness;
pub(crate) use history::{index_git_history, resolve_head_commit};
pub(crate) use merge::merge_git_incremental;

pub struct GitIndexRequest {
    pub repo_path: PathBuf,
    pub rebuild: bool,
    pub verbose: bool,
}

#[derive(Debug)]
pub enum GitIndexOutcome {
    Aborted,
    UpToDate,
    NoDocuments,
    Indexed {
        rebuilt: bool,
        chunk_count: usize,
        doc_count: usize,
        new_commit_count: usize,
        walk_secs: f64,
        embed_secs: f64,
    },
}

impl GitIndexOutcome {
    pub(crate) fn format_for_ui(&self) -> Vec<(&'static str, String)> {
        match self {
            GitIndexOutcome::Aborted => vec![("info", "Aborted.".to_string())],
            GitIndexOutcome::UpToDate => {
                vec![("info", "Git index is up to date.".to_string())]
            }
            GitIndexOutcome::NoDocuments => {
                vec![("info", "No git documents found.".to_string())]
            }
            GitIndexOutcome::Indexed { rebuilt, chunk_count, doc_count, new_commit_count, walk_secs, embed_secs } => {
                if *rebuilt {
                    vec![("info", format!(
                        "Git index written: {} chunks from {} docs (walk: {:.1}s, embed: {:.1}s)",
                        chunk_count, doc_count, walk_secs, embed_secs
                    ))]
                } else {
                    vec![("info", format!(
                        "Git index updated: {} chunks from {} docs ({} new commits, walk: {:.1}s, embed: {:.1}s)",
                        chunk_count, doc_count, new_commit_count, walk_secs, embed_secs
                    ))]
                }
            }
        }
    }
}

pub trait GitIndexer: Send + Sync {
    fn run(&self, config: &Config, request: GitIndexRequest) -> anyhow::Result<GitIndexOutcome>;
}

pub struct GitIndexerImpl {
    pub console: Box<dyn Console>,
}

pub fn create_git_indexer(console: Box<dyn Console>) -> impl GitIndexer {
    GitIndexerImpl { console }
}

impl GitIndexer for GitIndexerImpl {
    fn run(&self, config: &Config, request: GitIndexRequest) -> anyhow::Result<GitIndexOutcome> {
        let git_config = config.git.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "[git] section required in docent.toml for index-git. Please add it and try again."
            )
        })?;

        let persist_path = config.persist_path_buf();
        let dims = dims_for_model(&config.index.embedding_model)?;

        if request.rebuild {
            self.rebuild(&request, git_config, &persist_path, dims, config)
        } else {
            let repo = IndexRepository::new(&persist_path, &config.index);
            if !repo.exists(SourceIndexKind::Git) {
                anyhow::bail!(
                    "No existing Git index found at '{}'. Use `docent index-git --rebuild` to create one.",
                    persist_path.display()
                );
            }
            self.incremental(&request, git_config, &persist_path, dims, config)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures::{make_temp_dir, RecordingUi};

    fn base_config(persist: &std::path::Path) -> Config {
        let mut config = Config::default();
        config.index.persist_path = persist.to_string_lossy().to_string();
        config
    }

    #[test]
    fn missing_git_config_returns_error() {
        let persist = make_temp_dir("git_missing_config");
        let config = base_config(&persist);
        let ui = RecordingUi::always_confirm();
        let indexer = GitIndexerImpl {
            console: Box::new(ui),
        };
        let req = GitIndexRequest {
            repo_path: persist.clone(),
            rebuild: false,
            verbose: false,
        };
        let result = indexer.run(&config, req);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("[git] section"));
        let _ = std::fs::remove_dir_all(&persist);
    }
}
