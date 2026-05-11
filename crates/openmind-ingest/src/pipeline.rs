//! 默认摄入管道实现

use async_trait::async_trait;
use openmind_core::{
    ContentItem, IngestionPipeline, KnowledgeEntry, KnowledgeStore,
    EmbeddingModel, EntryMetadata,
};
use chrono::Utc;
use uuid::Uuid;

/// 默认摄入管道
///
/// 串联解析、分块、嵌入、索引、关联各阶段。
pub struct DefaultIngestionPipeline<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    embedding: E,
    store: K,
}

impl<E, K> DefaultIngestionPipeline<E, K>
where
    E: EmbeddingModel,
    K: KnowledgeStore,
{
    pub fn new(embedding: E, store: K) -> Self {
        Self { embedding, store }
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

        // 1. 分块（简单按段落分割，后续可替换为语义分块）
        let chunks = chunk_by_paragraph(&item.content);

        for chunk in chunks {
            if chunk.trim().is_empty() {
                continue;
            }

            // 2. 嵌入
            let _embedding = self.embedding.embed_text(&chunk).await?;

            // 3. 构建知识条目
            let entry = KnowledgeEntry {
                id: Uuid::new_v4().to_string(),
                source: item.source.clone(),
                content: chunk.to_string(),
                embedding_id: None, // Will be set by store
                metadata: EntryMetadata {
                    content_type: item.content_type.clone(),
                    url: item.metadata.url.clone(),
                    author: item.metadata.author.clone(),
                    project: item.metadata.project.clone(),
                    extra: item.metadata.extra.clone(),
                },
                tags: item.tags.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            // 4. 存储
            let id = self.store.store(entry).await?;
            ids.push(id);
        }

        Ok(ids)
    }
}

/// 按段落分割文本
fn chunk_by_paragraph(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect()
}
