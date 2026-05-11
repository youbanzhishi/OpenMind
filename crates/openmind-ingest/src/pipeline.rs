//! 默认摄入管道实现
//!
//! 管道: 解析→分块→嵌入(可选)→索引→存储
//! 降级路径：模型不可用时跳过嵌入，只做关键词索引

use async_trait::async_trait;
use openmind_core::{
    compute_content_hash, ContentItem, EmbeddingModel, EmbeddingStatus,
    EntryStatus, IngestionPipeline, KnowledgeEntry, KnowledgeStore,
    SourceType,
};
use chrono::Utc;
use uuid::Uuid;

use crate::chunker::{Chunker, ChunkingStrategy};
use crate::parser::ParserRegistry;

/// 默认摄入管道
///
/// 串联解析、分块、嵌入、索引、关联各阶段。
/// 支持降级模式：嵌入模型不可用时跳过嵌入步骤。
pub struct DefaultIngestionPipeline<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    embedding: E,
    store: K,
    parser: ParserRegistry,
    chunker: Chunker,
    /// 是否允许降级（跳过嵌入）
    allow_degradation: bool,
}

impl<E, K> DefaultIngestionPipeline<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    pub fn new(embedding: E, store: K) -> Self {
        Self {
            embedding,
            store,
            parser: ParserRegistry::new(),
            chunker: Chunker::default_paragraph(),
            allow_degradation: true,
        }
    }

    pub fn with_chunking_strategy(mut self, strategy: ChunkingStrategy) -> Self {
        self.chunker = Chunker::new(strategy);
        self
    }

    pub fn with_degradation(mut self, allow: bool) -> Self {
        self.allow_degradation = allow;
        self
    }

    /// 推断source_type
    fn infer_source_type(source: &str) -> SourceType {
        if source.contains("blog") || source.contains("post") {
            SourceType::Blog
        } else if source.contains("vault") {
            SourceType::Vault
        } else if source.contains("bookmark") {
            SourceType::Bookmark
        } else if source.contains("note") {
            SourceType::Note
        } else {
            SourceType::File
        }
    }

    /// 推断content_type
    fn infer_content_type(source: &str) -> String {
        if source.ends_with(".md") || source.ends_with(".markdown") {
            "markdown".to_string()
        } else if source.ends_with(".html") || source.ends_with(".htm") {
            "html".to_string()
        } else if source.ends_with(".rs") || source.ends_with(".py") || source.ends_with(".js") {
            "code".to_string()
        } else {
            "text".to_string()
        }
    }
}

#[async_trait]
impl<E, K> IngestionPipeline for DefaultIngestionPipeline<E, K>
where
    E: EmbeddingModel + 'static,
    K: KnowledgeStore + 'static,
{
    async fn ingest(&self, item: ContentItem) -> anyhow::Result<Vec<String>> {
        let mut ids = Vec::new();
        let now = Utc::now();

        // Determine content_type for chunking
        let content_type = if item.content_type.is_empty() {
            Self::infer_content_type(&item.source)
        } else {
            item.content_type.clone()
        };

        // 1. Parse if needed (the item may already be parsed)
        let parsed_item = if item.content.is_empty() {
            // Try to parse from raw content
            self.parser.parse(&item.source, &item.content, &content_type).await?
        } else {
            item
        };

        // 2. Chunk
        let parent_id = Uuid::new_v4().to_string();
        let chunks = self.chunker.chunk(&parsed_item.content, &content_type, &parent_id);

        if chunks.is_empty() {
            // No chunks, create a single entry from the full content
            let entry = self.create_entry(
                &parent_id,
                &parsed_item,
                &content_type,
                now,
            ).await?;
            let id = self.store.store(entry).await?;
            ids.push(id);
            return Ok(ids);
        }

        // 3. Process each chunk
        for chunk in chunks {
            let entry_id = chunk.id.clone();

            // 3a. Try embedding (with degradation)
            let (embedding_id, embedding_status) = if self.allow_degradation {
                match self.embedding.embed_text(&chunk.content).await {
                    Ok(_vec) => {
                        // In a full implementation, we'd store the vector in Qdrant
                        // and get back the vector ID. For now, mark as embedded.
                        (None, EmbeddingStatus::Embedded)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Embedding failed for chunk {}, marking as Pending: {}",
                            chunk.id, e
                        );
                        (None, EmbeddingStatus::Pending)
                    }
                }
            } else {
                match self.embedding.embed_text(&chunk.content).await {
                    Ok(_vec) => (None, EmbeddingStatus::Embedded),
                    Err(e) => return Err(e),
                }
            };

            // 3b. Build knowledge entry
            let entry = KnowledgeEntry {
                id: entry_id,
                source_type: Self::infer_source_type(&parsed_item.source),
                source_id: parsed_item.source.clone(),
                title: parsed_item.title.clone().unwrap_or_default(),
                content_hash: compute_content_hash(&chunk.content),
                content: chunk.content,
                embedding_id,
                embedding_status,
                tags: parsed_item.tags.clone(),
                project: None,
                metadata: parsed_item.metadata.clone(),
                file_references: parsed_item.file_references.clone(),
                created_at: now,
                updated_at: now,
                status: EntryStatus::Active,
            };

            // 3c. Store
            let id = self.store.store(entry).await?;
            ids.push(id);
        }

        Ok(ids)
    }
}

impl<E, K> DefaultIngestionPipeline<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    /// Create a single knowledge entry from a ContentItem (no chunking)
    async fn create_entry(
        &self,
        id: &str,
        item: &ContentItem,
        _content_type: &str,
        now: chrono::DateTime<Utc>,
    ) -> anyhow::Result<KnowledgeEntry> {
        let content_hash = compute_content_hash(&item.content);

        let (embedding_id, embedding_status) = if self.allow_degradation {
            match self.embedding.embed_text(&item.content).await {
                Ok(_) => (None, EmbeddingStatus::Embedded),
                Err(e) => {
                    tracing::warn!("Embedding failed, marking as Pending: {}", e);
                    (None, EmbeddingStatus::Pending)
                }
            }
        } else {
            match self.embedding.embed_text(&item.content).await {
                Ok(_) => (None, EmbeddingStatus::Embedded),
                Err(e) => return Err(e),
            }
        };

        Ok(KnowledgeEntry {
            id: id.to_string(),
            source_type: Self::infer_source_type(&item.source),
            source_id: item.source.clone(),
            title: item.title.clone().unwrap_or_default(),
            content: item.content.clone(),
            content_hash,
            embedding_id,
            embedding_status,
            tags: item.tags.clone(),
            project: None,
            metadata: item.metadata.clone(),
            file_references: item.file_references.clone(),
            created_at: now,
            updated_at: now,
            status: EntryStatus::Active,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmind_core::{IngestionPipeline, DummyEmbeddingModel, SqliteKnowledgeStore};

    #[tokio::test]
    async fn test_pipeline_ingest() {
        let embedding = DummyEmbeddingModel::new(64);
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let pipeline = DefaultIngestionPipeline::new(embedding, store);

        let item = ContentItem {
            source: "test.md".to_string(),
            content_type: "markdown".to_string(),
            content: "# Hello\n\nWorld of Rust.\n\n## Section\n\nMore content here.".to_string(),
            title: Some("Test".to_string()),
            metadata: serde_json::json!({}),
            file_references: vec![],
            tags: vec!["test".to_string()],
        };

        let ids = pipeline.ingest(item).await.unwrap();
        assert!(!ids.is_empty(), "Should produce at least one entry ID");
    }

    #[tokio::test]
    async fn test_pipeline_with_degradation() {
        // Use a failing embedding model for testing degradation
        struct FailingEmbeddingModel;
        
        #[async_trait]
        impl EmbeddingModel for FailingEmbeddingModel {
            fn model_name(&self) -> &str { "failing" }
            fn dimension(&self) -> usize { 64 }
            async fn embed_text(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
                anyhow::bail!("Model unavailable for testing")
            }
        }

        let embedding = FailingEmbeddingModel;
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let pipeline = DefaultIngestionPipeline::new(embedding, store);

        let item = ContentItem {
            source: "test.txt".to_string(),
            content_type: "text".to_string(),
            content: "This is a test entry about Rust.".to_string(),
            title: Some("Degraded Test".to_string()),
            metadata: serde_json::json!({}),
            file_references: vec![],
            tags: vec![],
        };

        // Should succeed even when embedding fails (degradation)
        let ids = pipeline.ingest(item).await.unwrap();
        assert!(!ids.is_empty());

        // Verify the entry is stored with Pending embedding status
        // (can't easily verify through pipeline's store since it's consumed)
    }
}
