use std::path::Path;
use std::time::Instant;
use crate::app::index::runner;
use crate::config::{Config, GitConfig};
use crate::index::{IndexRepository, SourceIndexKind};
use crate::app::index::pipeline::unique_doc_count;
use super::{GitIndexOutcome, GitIndexRequest, GitIndexerImpl};
impl GitIndexerImpl {
    fn walk_commits(
        &self,
        request: &GitIndexRequest,
        git_config: &GitConfig,
        total_est: usize,
    ) -> anyhow::Result<(Vec<crate::app::index::git::extract::GitDocument>, f64)> {
        let walk_start = Instant::now();
        let pb_walk = self.console.progress(total_est as u64, "Walking commits");
        let docs = super::index_git_history(
            &request.repo_path, git_config, None, true, request.verbose, Some(pb_walk.as_ref()),
        )?;
        pb_walk.finish();
        let walk_secs = walk_start.elapsed().as_secs_f64();
        Ok((docs, walk_secs))
    }
    fn embed_docs(
        &self,
        docs: &[crate::app::index::git::extract::GitDocument],
        _request: &GitIndexRequest,
        config: &Config,
    ) -> anyhow::Result<(crate::app::index::pipeline::IndexedBatch, usize, f64)> {
        let total_docs = docs.len();
        let embed_start = Instant::now();
        let pb_embed = self.console.progress(total_docs as u64, "Embedding");
        let freshness = super::compute_freshness(docs);
        let indexable = super::prepare_git_documents(docs, &freshness);
        let (batch, dims) = runner::run_indexing_pipeline(
            &config.index,
            &indexable,
            config.search.bm25.k1,
            config.search.bm25.b,
            Some(pb_embed.as_ref()),
        )?;
        pb_embed.finish();
        let embed_secs = embed_start.elapsed().as_secs_f64();
        Ok((batch, dims, embed_secs))
    }
    pub(super) fn rebuild(
        &self,
        request: &GitIndexRequest,
        git_config: &GitConfig,
        persist_path: &Path,
        dims: usize,
        config: &Config,
    ) -> anyhow::Result<GitIndexOutcome> {
        let total_est = match self.check_git_size(&request.repo_path, git_config, dims, None, config)? {
            Some(n) => n,
            None => return Ok(GitIndexOutcome::Aborted),
        };
        let (docs, walk_secs) = self.walk_commits(request, git_config, total_est)?;
        if docs.is_empty() {
            return Ok(GitIndexOutcome::NoDocuments);
        }
        let head_commit = super::resolve_head_commit(&request.repo_path, &git_config.branch)?;
        let (batch, dims, embed_secs) = self.embed_docs(&docs, request, config)?;
        let repo = IndexRepository::new(persist_path, &config.index);
        let chunk_count = batch.metadata.len();
        let doc_count = unique_doc_count(&batch.metadata);
        repo.store(SourceIndexKind::Git, &batch, dims, doc_count, Some(head_commit))?;
        Ok(GitIndexOutcome::Indexed {
            rebuilt: true, chunk_count, doc_count,
            new_commit_count: docs.len(), walk_secs, embed_secs,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::super::GitIndexer;
    use crate::tests::fixtures::{make_temp_dir, RecordingUi};
    #[test]
    fn rebuild_without_git_section_returns_error() {
        let persist = make_temp_dir("git_rebuild_no_git");
        let config = crate::config::Config::default();
        let ui = RecordingUi::always_confirm();
        let indexer = super::GitIndexerImpl {
            console: Box::new(ui),
        };
        let req = super::GitIndexRequest {
            repo_path: persist.clone(),
            rebuild: true,
            verbose: false,
        };
        let result = indexer.run(&config, req);
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&persist);
    }
}
