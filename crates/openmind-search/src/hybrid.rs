//! 混合搜索引擎
//!
//! 融合关键词搜索和语义搜索的结果，按加权分数排序。
//! 当语义搜索不可用时，自动降级为纯关键词搜索。

use openmind_core::{
    EmbeddingModel, KnowledgeStore, SearchMode, SearchRequest, SearchResponse, SearchResult,
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

    /// 执行搜索（带降级支持）
    pub async fn search(&self, request: SearchRequest) -> anyhow::Result<SearchResponse> {
        let limit = request.limit.unwrap_or(10);
        let filters = request.filters.clone();

        match request.mode {
            SearchMode::Keyword => {
                let results = self
                    .store
                    .query_keyword(&request.query, limit, &filters)
                    .await?;
                let total = results.len();
                Ok(SearchResponse {
                    results,
                    mode: SearchMode::Keyword,
                    total,
                    degraded: false,
                })
            }
            SearchMode::Semantic => {
                match self.embedding.embed_text(&request.query).await {
                    Ok(embedding) => {
                        let results = self
                            .store
                            .query_semantic(&request.query, &embedding, limit)
                            .await?;
                        let total = results.len();
                        Ok(SearchResponse {
                            results,
                            mode: SearchMode::Semantic,
                            total,
                            degraded: false,
                        })
                    }
                    Err(e) => {
                        tracing::warn!("Semantic search degraded, falling back to keyword: {}", e);
                        // 降级：回退到关键词搜索
                        let results = self
                            .store
                            .query_keyword(&request.query, limit, &filters)
                            .await?;
                        let total = results.len();
                        Ok(SearchResponse {
                            results,
                            mode: SearchMode::Keyword,
                            total,
                            degraded: true,
                        })
                    }
                }
            }
            SearchMode::Hybrid => {
                let keyword_results = self
                    .store
                    .query_keyword(&request.query, limit * 2, &filters)
                    .await?;

                let semantic_results = match self.embedding.embed_text(&request.query).await {
                    Ok(embedding) => {
                        self.store
                            .query_semantic(&request.query, &embedding, limit * 2)
                            .await?
                    }
                    Err(e) => {
                        tracing::warn!("Semantic search unavailable in hybrid mode: {}", e);
                        vec![]
                    }
                };

                let is_degraded = semantic_results.is_empty();

                // Simple fusion: merge and deduplicate by entry ID
                let mut merged: Vec<SearchResult> = Vec::new();
                let keyword_weight = 1.0 - self.semantic_weight;

                for kr in keyword_results {
                    if let Some(existing) = merged.iter_mut().find(|r| r.entry.id == kr.entry.id) {
                        existing.relevance = existing.relevance * self.semantic_weight
                            + kr.relevance * keyword_weight;
                    } else {
                        let mut r = kr.clone();
                        r.relevance = r.relevance * keyword_weight;
                        merged.push(r);
                    }
                }

                for sr in semantic_results {
                    if let Some(existing) = merged.iter_mut().find(|r| r.entry.id == sr.entry.id) {
                        existing.relevance = existing.relevance * self.semantic_weight
                            + sr.relevance * keyword_weight;
                    } else {
                        let mut r = sr.clone();
                        r.relevance = r.relevance * self.semantic_weight;
                        merged.push(r);
                    }
                }

                merged.sort_by(|a, b| {
                    b.relevance
                        .partial_cmp(&a.relevance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                merged.truncate(limit);

                let total = merged.len();
                Ok(SearchResponse {
                    results: merged,
                    mode: if is_degraded {
                        SearchMode::Keyword
                    } else {
                        SearchMode::Hybrid
                    },
                    total,
                    degraded: is_degraded,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use openmind_core::{
        compute_content_hash, DummyEmbeddingModel, EmbeddingStatus, EntryStatus, SearchFilters,
        SourceType, SqliteKnowledgeStore,
    };
    use uuid::Uuid;

    async fn setup_test_data() -> (DummyEmbeddingModel, SqliteKnowledgeStore) {
        let embedding = DummyEmbeddingModel::new(64);
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let now = Utc::now();

        // Insert test entries
        let entries = vec![
            (
                "Rust Programming Guide",
                "Rust is a systems programming language focused on safety and performance.",
            ),
            (
                "Python Data Science",
                "Python is widely used for data science and machine learning.",
            ),
            (
                "Rust vs Go Comparison",
                "Comparing Rust and Go for backend systems development.",
            ),
        ];

        for (title, content) in entries {
            let entry = openmind_core::KnowledgeEntry {
                id: Uuid::new_v4().to_string(),
                source_type: SourceType::File,
                source_id: format!("{}.md", title.to_lowercase().replace(' ', "-")),
                title: title.to_string(),
                content: content.to_string(),
                content_hash: compute_content_hash(content),
                embedding_id: None,
                embedding_status: EmbeddingStatus::Pending,
                tags: vec![],
                project: None,
                metadata: serde_json::json!({}),
                file_references: vec![],
                created_at: now,
                updated_at: now,
                status: EntryStatus::Active,
            };
            store.store(entry).await.unwrap();
        }

        (embedding, store)
    }

    #[tokio::test]
    async fn test_keyword_search() {
        let (embedding, store) = setup_test_data().await;
        let engine = HybridSearchEngine::new(embedding, store, 0.5);

        let request = SearchRequest {
            query: "Rust".to_string(),
            mode: SearchMode::Keyword,
            limit: Some(10),
            filters: SearchFilters::default(),
        };

        let response = engine.search(request).await.unwrap();
        assert!(response.results.len() >= 2);
        assert_eq!(response.mode, SearchMode::Keyword);
        assert!(!response.degraded);
    }

    #[tokio::test]
    async fn test_semantic_degradation() {
        // Test with a failing embedding model
        struct FailingEmbedding;

        #[async_trait::async_trait]
        impl openmind_core::EmbeddingModel for FailingEmbedding {
            fn model_name(&self) -> &str {
                "failing"
            }
            fn dimension(&self) -> usize {
                64
            }
            async fn embed_text(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
                anyhow::bail!("Model unavailable")
            }
        }

        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let embedding = FailingEmbedding;
        let engine = HybridSearchEngine::new(embedding, store, 0.5);

        let request = SearchRequest {
            query: "programming".to_string(),
            mode: SearchMode::Semantic,
            limit: Some(10),
            filters: SearchFilters::default(),
        };

        // Semantic search should degrade when embedding fails
        let response = engine.search(request).await.unwrap();
        assert!(response.degraded);
        assert_eq!(response.mode, SearchMode::Keyword);
    }
}
