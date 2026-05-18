mod types;
mod ranking;
mod fusion;
mod backend;
mod orchestrator;

use std::sync::Arc;
use std::sync::Mutex;

use crate::config::SearchConfig;
use crate::index::embedder::Embedder;
use crate::index::MergedIndex;

pub use types::SearchResult;

pub(super) use fusion::create_fusion;

pub(super) use ranking::DecayRanker;

use backend::build_backends;
use orchestrator::HybridSearchService;

#[async_trait::async_trait]
pub trait SearchService: Send + Sync {
    async fn search(
        &self,
        query: &str,
        limit: usize,
        file_hint: &str,
    ) -> anyhow::Result<Vec<SearchResult>>;
}

pub fn create_search_service(
    merged: MergedIndex,
    embedder: Arc<Mutex<dyn Embedder>>,
    search_config: &SearchConfig,
) -> anyhow::Result<Arc<dyn SearchService>> {
    let (semantic_backend, bm25_backend) = build_backends(&merged, embedder);

    let fusion = create_fusion(
        &search_config.fusion.strategy,
        search_config.fusion.rrf_k,
        search_config.fusion.semantic_weight,
    )?;

    let ranker = Arc::new(DecayRanker::new(
        search_config.ranking.same_src_score_decay,
        search_config.ranking.file_hint_boost,
    ));

    let svc = HybridSearchService {
        semantic_backend,
        bm25_backend,
        fusion,
        ranker,
        metadata: Arc::new(merged.metadata),
        index_time: merged.built_at,
    };

    Ok(Arc::new(svc) as Arc<dyn SearchService>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SearchConfig, FusionConfig, RankingConfig, Bm25Config};
    use crate::index::MergedIndex;
    use crate::index::VectorStore;
    use crate::tests::mock_embedder::mock_embedder;

    fn default_search_config() -> SearchConfig {
        SearchConfig {
            ranking: RankingConfig {
                same_src_score_decay: 0.9,
                file_hint_boost: 1.5,
            },
            fusion: FusionConfig {
                strategy: "rrf".to_string(),
                rrf_k: 60.0,
                semantic_weight: 0.7,
            },
            bm25: Bm25Config {
                k1: 1.2,
                b: 0.75,
            },
        }
    }

    #[test]
    fn test_build_hybrid_search_service_without_bm25() {
        let merged = MergedIndex {
            vectors: VectorStore::from_vec_vec(vec![vec![1.0, 2.0, 3.0]]).unwrap(),
            metadata: vec![],
            bm25_embeddings: None,
            bm25_header: None,
            built_at: "now".to_string(),
        };
        let embedder: Arc<Mutex<dyn Embedder>> =
            Arc::new(Mutex::new(mock_embedder()));
        let search_config = default_search_config();
        let result = create_search_service(merged, embedder, &search_config);
        assert!(result.is_ok());
    }


}
