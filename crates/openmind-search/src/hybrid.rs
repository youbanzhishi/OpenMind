//! 混合搜索引擎
//!
//! 融合关键词搜索和语义搜索的结果，按加权分数排序。

use openmind_core::{
    EmbeddingModel, KnowledgeStore, SearchMode, SearchRequest,
    SearchResponse, SearchResult,
};

/// 混合搜索引擎
///
/// 将关键词搜索和语义搜索结果融合，支持可配置的权重。
pub struct HybridSearchEngine<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    embedding: E,
    store: K,
    /// 语义搜索权重（0.0-1.0，剩余为关键词权重）
    semantic_weight: f64,
}

impl<E, K> HybridSearchEngine<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    pub fn new(embedding: E, store: K, semantic_weight: f64) -> Self {
        Self {
            embedding,
            store,
            semantic_weight: semantic_weight.clamp(0.0, 1.0),
        }
    }
}

impl<E, K> HybridSearchEngine<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    /// 执行搜索
    pub async fn search(&self, request: SearchRequest) -> anyhow::Result<SearchResponse> {
        let limit = request.limit.unwrap_or(10);

        match request.mode {
            SearchMode::Keyword => {
                let results = self.store.query_keyword(&request.query, limit).await?;
                Ok(SearchResponse {
                    results,
                    mode: SearchMode::Keyword,
                    total: 0,
                })
            }
            SearchMode::Semantic => {
                let embedding = self.embedding.embed_text(&request.query).await?;
                let results = self.store.query_semantic(&request.query, &embedding, limit).await?;
                Ok(SearchResponse {
                    results,
                    mode: SearchMode::Semantic,
                    total: 0,
                })
            }
            SearchMode::Hybrid => {
                let keyword_results = self.store.query_keyword(&request.query, limit * 2).await?;
                let embedding = self.embedding.embed_text(&request.query).await?;
                let semantic_results = self.store.query_semantic(&request.query, &embedding, limit * 2).await?;

                // Simple fusion: merge and deduplicate by entry ID
                let mut merged: Vec<SearchResult> = Vec::new();
                let keyword_weight = 1.0 - self.semantic_weight;

                for kr in keyword_results {
                    if let Some(existing) = merged.iter_mut().find(|r| r.entry.id == kr.entry.id) {
                        existing.relevance = existing.relevance * self.semantic_weight + kr.relevance * keyword_weight;
                    } else {
                        let mut r = kr.clone();
                        r.relevance = r.relevance * keyword_weight;
                        merged.push(r);
                    }
                }

                for sr in semantic_results {
                    if let Some(existing) = merged.iter_mut().find(|r| r.entry.id == sr.entry.id) {
                        existing.relevance = existing.relevance * self.semantic_weight + sr.relevance * keyword_weight;
                    } else {
                        let mut r = sr.clone();
                        r.relevance = r.relevance * self.semantic_weight;
                        merged.push(r);
                    }
                }

                merged.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
                merged.truncate(limit);

                let total = merged.len();
                Ok(SearchResponse {
                    results: merged,
                    mode: SearchMode::Hybrid,
                    total,
                })
            }
        }
    }
}
