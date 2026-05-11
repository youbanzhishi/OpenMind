//! RAG查询管道
//!
//! 检索→重排→生成 的完整RAG管道。
//! 支持多种检索策略和重排算法。

use openmind_core::{
    EmbeddingModel, KnowledgeStore, KnowledgeEntry, SearchResult,
    SearchFilters, SearchMode, VectorStore,
};
use serde::{Deserialize, Serialize};

/// RAG查询请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagRequest {
    /// 查询文本
    pub query: String,
    /// 返回的上下文条目数
    pub top_k: usize,
    /// 搜索模式（keyword/semantic/hybrid）
    pub search_mode: SearchMode,
    /// 相似度阈值
    pub similarity_threshold: f64,
    /// 是否包含元数据
    pub include_metadata: bool,
}

impl Default for RagRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            top_k: 5,
            search_mode: SearchMode::Hybrid,
            similarity_threshold: 0.3,
            include_metadata: true,
        }
    }
}

/// RAG查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResponse {
    /// 检索到的上下文条目
    pub contexts: Vec<RagContext>,
    /// 重排后的查询建议
    pub suggested_queries: Vec<String>,
    /// 是否降级
    pub degraded: bool,
}

/// RAG上下文条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagContext {
    /// 知识条目
    pub entry: KnowledgeEntry,
    /// 相关度分数
    pub relevance: f64,
    /// 上下文片段（截取自内容）
    pub snippet: String,
}

/// 重排策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerankStrategy {
    /// 按相关度分数排序
    ScoreOnly,
    /// 考虑新鲜度的混合排序
    FreshnessWeighted,
    /// 多因子排序（相关度+新鲜度+权威度）
    MultiFactor,
}

/// RAG查询管道
///
/// 完整的检索增强生成管道：
/// 1. 检索：根据查询从知识库中检索相关条目
/// 2. 重排：对检索结果进行重新排序
/// 3. 上下文构建：构建适合LLM消费的上下文
pub struct RagPipeline<E, V, K>
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
    /// 重排策略
    rerank_strategy: RerankStrategy,
    /// 语义搜索权重
    semantic_weight: f64,
}

impl<E, V, K> RagPipeline<E, V, K>
where
    E: EmbeddingModel,
    V: VectorStore,
    K: KnowledgeStore,
{
    /// 创建RAG管道
    pub fn new(embedding: E, vector_store: V, store: K) -> Self {
        Self {
            embedding,
            vector_store,
            store,
            rerank_strategy: RerankStrategy::ScoreOnly,
            semantic_weight: 0.6,
        }
    }

    /// 设置重排策略
    pub fn with_rerank_strategy(mut self, strategy: RerankStrategy) -> Self {
        self.rerank_strategy = strategy;
        self
    }

    /// 设置语义搜索权重
    pub fn with_semantic_weight(mut self, weight: f64) -> Self {
        self.semantic_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// 执行RAG查询
    pub async fn query(&self, request: RagRequest) -> anyhow::Result<RagResponse> {
        // 1. 检索阶段
        let search_results = self.retrieve(&request).await?;

        // 2. 重排阶段
        let reranked = self.rerank(search_results, &request);

        // 3. 上下文构建
        let contexts = self.build_contexts(reranked);

        // 4. 生成建议查询
        let suggested_queries = self.generate_suggestions(&contexts);

        Ok(RagResponse {
            contexts,
            suggested_queries,
            degraded: false,
        })
    }

    /// 检索阶段
    async fn retrieve(&self, request: &RagRequest) -> anyhow::Result<Vec<SearchResult>> {
        let limit = request.top_k * 3; // Retrieve more for reranking

        match request.search_mode {
            SearchMode::Keyword => {
                let filters = SearchFilters::default();
                self.store.query_keyword(&request.query, limit, &filters).await
            }
            SearchMode::Semantic => {
                let query_vector = self.embedding.embed_text(&request.query).await?;
                let vector_results = self.vector_store.search(
                    query_vector,
                    limit,
                    request.similarity_threshold,
                ).await?;

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
            SearchMode::Hybrid => {
                let keyword_weight = 1.0 - self.semantic_weight;

                // Keyword results
                let filters = SearchFilters::default();
                let keyword_results = self.store.query_keyword(&request.query, limit, &filters).await?;

                // Semantic results
                let semantic_results = match self.embedding.embed_text(&request.query).await {
                    Ok(query_vector) => {
                        match self.vector_store.search(query_vector, limit, request.similarity_threshold).await {
                            Ok(vector_results) => {
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
                                results
                            }
                            Err(_) => vec![],
                        }
                    }
                    Err(_) => vec![],
                };

                // Fuse results
                let mut merged: Vec<SearchResult> = Vec::new();
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

                Ok(merged)
            }
        }
    }

    /// 重排阶段
    fn rerank(&self, mut results: Vec<SearchResult>, request: &RagRequest) -> Vec<SearchResult> {
        match self.rerank_strategy {
            RerankStrategy::ScoreOnly => {
                results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
            }
            RerankStrategy::FreshnessWeighted => {
                // Combine score with freshness (newer = higher)
                results.sort_by(|a, b| {
                    let score_a = a.relevance * 0.8 + freshness_score(&a.entry.updated_at) * 0.2;
                    let score_b = b.relevance * 0.8 + freshness_score(&b.entry.updated_at) * 0.2;
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            RerankStrategy::MultiFactor => {
                results.sort_by(|a, b| {
                    let score_a = a.relevance * 0.6
                        + freshness_score(&a.entry.updated_at) * 0.2
                        + authority_score(&a.entry) * 0.2;
                    let score_b = b.relevance * 0.6
                        + freshness_score(&b.entry.updated_at) * 0.2
                        + authority_score(&b.entry) * 0.2;
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
        results.truncate(request.top_k);
        results
    }

    /// 上下文构建
    fn build_contexts(&self, results: Vec<SearchResult>) -> Vec<RagContext> {
        results
            .into_iter()
            .map(|r| {
                let snippet = build_snippet(&r.entry.content, 500);
                RagContext {
                    entry: r.entry,
                    relevance: r.relevance,
                    snippet,
                }
            })
            .collect()
    }

    /// 生成建议查询（基于检索到的上下文中的标签和关键词）
    fn generate_suggestions(&self, contexts: &[RagContext]) -> Vec<String> {
        let mut suggestions = Vec::new();
        for ctx in contexts.iter().take(3) {
            for tag in &ctx.entry.tags {
                let suggestion = format!("{} {}", ctx.entry.title, tag);
                if !suggestions.contains(&suggestion) {
                    suggestions.push(suggestion);
                }
            }
        }
        suggestions.truncate(5);
        suggestions
    }
}

/// 计算新鲜度分数（0.0-1.0，越新越高）
fn freshness_score(updated_at: &chrono::DateTime<chrono::Utc>) -> f64 {
    use chrono::Utc;
    let age_hours = (Utc::now() - *updated_at).num_hours().max(0) as f64;
    // Exponential decay: half-life of 30 days
    (0.5_f64).powf(age_hours / (30.0 * 24.0))
}

/// 计算权威度分数（0.0-1.0）
fn authority_score(entry: &KnowledgeEntry) -> f64 {
    let mut score = 0.5; // Base score
    // More tags = more curated = more authoritative
    score += (entry.tags.len() as f64 * 0.05).min(0.2);
    // Has project = more organized
    if entry.project.is_some() {
        score += 0.1;
    }
    // Has file references = more evidence
    score += (entry.file_references.len() as f64 * 0.05).min(0.2);
    score.min(1.0)
}

/// 构建上下文片段
fn build_snippet(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        let snippet: String = content.chars().take(max_len).collect();
        format!("{}...", snippet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmind_core::{
        DummyEmbeddingModel, InMemoryVectorStore, SqliteKnowledgeStore,
        compute_content_hash, EmbeddingStatus, EntryStatus, SourceType, VectorPoint,
    };
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    async fn setup_rag_pipeline() -> (DummyEmbeddingModel, InMemoryVectorStore, SqliteKnowledgeStore) {
        let embedding = DummyEmbeddingModel::new(64);
        let vector_store = InMemoryVectorStore::new(64);
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();

        let now = Utc::now();
        let entries = vec![
            ("Rust Programming Guide", "Rust is a systems programming language focused on safety and performance.", vec!["rust", "programming"]),
            ("Python Data Science", "Python is widely used for data science and machine learning applications.", vec!["python", "data-science"]),
            ("Rust vs Go", "Comparing Rust and Go for backend systems development.", vec!["rust", "go", "comparison"]),
        ];

        for (title, content, tags) in entries {
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
                tags: tags.into_iter().map(String::from).collect(),
                project: Some("test-project".to_string()),
                metadata: serde_json::json!({}),
                file_references: vec![],
                created_at: now,
                updated_at: now,
                status: EntryStatus::Active,
            };
            store.store(entry).await.unwrap();

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
    async fn test_rag_keyword_mode() {
        let (embedding, vector_store, store) = setup_rag_pipeline().await;
        let pipeline = RagPipeline::new(embedding, vector_store, store);

        let request = RagRequest {
            query: "Rust programming".to_string(),
            top_k: 3,
            search_mode: SearchMode::Keyword,
            similarity_threshold: 0.3,
            include_metadata: true,
        };

        let response = pipeline.query(request).await.unwrap();
        assert!(!response.contexts.is_empty(), "Should find contexts");
    }

    #[tokio::test]
    async fn test_rag_hybrid_mode() {
        let (embedding, vector_store, store) = setup_rag_pipeline().await;
        let pipeline = RagPipeline::new(embedding, vector_store, store);

        let request = RagRequest {
            query: "programming language".to_string(),
            top_k: 3,
            search_mode: SearchMode::Hybrid,
            similarity_threshold: 0.3,
            include_metadata: true,
        };

        let response = pipeline.query(request).await.unwrap();
        assert!(!response.contexts.is_empty());
    }

    #[tokio::test]
    async fn test_rag_rerank_strategies() {
        for strategy in [RerankStrategy::ScoreOnly, RerankStrategy::FreshnessWeighted, RerankStrategy::MultiFactor] {
            let (e, v, s) = setup_rag_pipeline().await;
            let pipeline = RagPipeline::new(e, v, s).with_rerank_strategy(strategy);

            let request = RagRequest {
                query: "Rust".to_string(),
                top_k: 2,
                search_mode: SearchMode::Keyword,
                similarity_threshold: 0.3,
                include_metadata: true,
            };

            let response = pipeline.query(request).await.unwrap();
            assert!(!response.contexts.is_empty(), "Strategy {:?} should find contexts", strategy);
        }
    }

    #[test]
    fn test_build_snippet() {
        let short = "Hello world";
        assert_eq!(build_snippet(short, 500), short);

        let long = "A".repeat(1000);
        let snippet = build_snippet(&long, 100);
        assert!(snippet.len() <= 103); // 100 chars + "..."
        assert!(snippet.ends_with("..."));
    }
}
