//! 语义搜索引擎
//!
//! 基于向量相似度的语义搜索，通过VectorStore进行向量检索。

use openmind_core::{SearchFilters,
    EmbeddingModel, KnowledgeStore, SearchMode,
    SearchRequest, SearchResponse, SearchResult, VectorStore,
};

/// 语义搜索引擎
///
/// 将查询文本向量化后，通过VectorStore进行相似度搜索，
/// 再从KnowledgeStore获取完整的知识条目。
pub struct SemanticSearchEngine<E, V, K>
where
    E: EmbeddingModel,
    V: VectorStore,
    K: KnowledgeStore,
{
    /// 嵌入模型
    embedding: E,
    /// 向量存储
    vector_store: V,
    /// 知识存储
    store: K,
    /// 相似度阈值
    threshold: f64,
}

impl<E, V, K> SemanticSearchEngine<E, V, K>
where
    E: EmbeddingModel,
    V: VectorStore,
    K: KnowledgeStore,
{
    /// 创建语义搜索引擎
    pub fn new(embedding: E, vector_store: V, store: K) -> Self {
        Self {
            embedding,
            vector_store,
            store,
            threshold: 0.5,
        }
    }

    /// 设置相似度阈值
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// 执行语义搜索
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        // 1. 向量化查询
        let query_vector = self.embedding.embed_text(query).await?;

        // 2. 向量相似度搜索
        let vector_results = self.vector_store.search(query_vector, limit, self.threshold).await?;

        // 3. 获取完整的知识条目
        let mut results = Vec::with_capacity(vector_results.len());
        for vr in vector_results {
            if let Some(entry_id) = vr.metadata.get("entry_id") {
                if let Ok(Some(entry)) = self.store.get(entry_id).await {
                    results.push(SearchResult {
                        entry,
                        relevance: vr.score,
                        highlights: vec![],
                    });
                }
            }
        }

        Ok(results)
    }

    /// 执行搜索并返回SearchResponse
    pub async fn search_response(
        &self,
        request: SearchRequest,
    ) -> anyhow::Result<SearchResponse> {
        let limit = request.limit.unwrap_or(10);
        let results = self.search(&request.query, limit).await?;
        let total = results.len();
        Ok(SearchResponse {
            results,
            mode: SearchMode::Semantic,
            total,
            degraded: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmind_core::{SearchFilters,
        DummyEmbeddingModel, InMemoryVectorStore, KnowledgeEntry, SqliteKnowledgeStore,
        compute_content_hash, EmbeddingStatus, EntryStatus, SourceType, VectorPoint,
    };
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    async fn setup_semantic_search() -> (DummyEmbeddingModel, InMemoryVectorStore, SqliteKnowledgeStore) {
        let embedding = DummyEmbeddingModel::new(64);
        let vector_store = InMemoryVectorStore::new(64);
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();

        // Insert test data
        let now = Utc::now();
        let entries = vec![
            ("Rust Programming", "Rust is a systems programming language."),
            ("Python Data Science", "Python is widely used for data science."),
        ];

        for (title, content) in entries {
            let id = Uuid::new_v4().to_string();
            let entry = KnowledgeEntry {
                id: id.clone(),
                source_type: SourceType::File,
                source_id: format!("{}.md", title.to_lowercase().replace(' ', "-")),
                title: title.to_string(),
                content: content.to_string(),
                content_hash: compute_content_hash(content),
                embedding_id: Some(id.clone()),
                embedding_status: EmbeddingStatus::Embedded,
                tags: vec![],
                project: None,
                metadata: serde_json::json!({}),
                file_references: vec![],
                created_at: now,
                updated_at: now,
                status: EntryStatus::Active,
            };
            store.store(entry).await.unwrap();

            // Insert vector
            let vec = embedding.embed_text(content).await.unwrap();
            vector_store.upsert(VectorPoint {
                id: id.clone(),
                vector: vec,
                metadata: HashMap::from([("entry_id".to_string(), id)]),
            }).await.unwrap();
        }

        (embedding, vector_store, store)
    }

    #[tokio::test]
    async fn test_semantic_search() {
        let (embedding, vector_store, store) = setup_semantic_search().await;
        let engine = SemanticSearchEngine::new(embedding, vector_store, store);

        let results = engine.search("programming language", 10).await.unwrap();
        // Dummy embedding returns same vector for all inputs, so all entries should match
        assert!(!results.is_empty(), "Should find results");
    }

    #[tokio::test]
    async fn test_semantic_search_response() {
        let (embedding, vector_store, store) = setup_semantic_search().await;
        let engine = SemanticSearchEngine::new(embedding, vector_store, store);

        let request = SearchRequest {
            query: "data science".to_string(),
            mode: SearchMode::Semantic,
            limit: Some(5),
            filters: SearchFilters::default(),
        };

        let response = engine.search_response(request).await.unwrap();
        assert_eq!(response.mode, SearchMode::Semantic);
        assert!(!response.degraded);
    }
}
