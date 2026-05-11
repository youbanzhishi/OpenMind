//! 核心Trait定义
//!
//! 所有可插拔组件的抽象接口。

use async_trait::async_trait;
use crate::models::*;

/// 数据源可插拔接口
///
/// 每个数据源（博客/Vault/书签/备忘录）实现此trait，
/// 即可被OpenMind摄入管道统一处理。
#[async_trait]
pub trait Connector: Send + Sync {
    /// Connector名称（如 "vault", "blog", "bookmark"）
    fn name(&self) -> &str;

    /// 建立连接（验证配置、初始化客户端）
    async fn connect(&self) -> anyhow::Result<()>;

    /// 列出自上次同步以来的变更
    async fn list_changes(&self, since: &SyncState) -> anyhow::Result<Vec<ContentChange>>;

    /// 获取指定内容的完整数据
    async fn fetch_content(&self, id: &str) -> anyhow::Result<ContentItem>;

    /// 执行完整同步（list_changes → fetch_content → 返回）
    async fn sync(&self, since: &SyncState) -> anyhow::Result<Vec<ContentItem>> {
        let changes = self.list_changes(since).await?;
        let mut items = Vec::with_capacity(changes.len());
        for change in changes {
            match change {
                ContentChange::Added(id) | ContentChange::Modified(id) => {
                    match self.fetch_content(&id).await {
                        Ok(item) => items.push(item),
                        Err(e) => tracing::warn!("Failed to fetch content {}: {}", id, e),
                    }
                }
                ContentChange::Deleted(_id) => {
                    // Deleted items handled separately by caller
                }
            }
        }
        Ok(items)
    }
}

/// 存储后端可插拔接口
///
/// 支持本地文件系统、S3、OpenVault等存储后端。
/// 大文件（图片/音频/视频）通过此接口存取。
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 存储后端名称
    fn name(&self) -> &str;

    /// 存储数据，返回存储ID
    async fn put(&self, key: &str, data: &[u8]) -> anyhow::Result<String>;

    /// 获取数据
    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>>;

    /// 删除数据
    async fn delete(&self, key: &str) -> anyhow::Result<()>;

    /// 获取可访问的URL（适用于大文件引用）
    async fn get_url(&self, key: &str) -> anyhow::Result<String>;
}

/// 嵌入模型可插拔接口
///
/// 支持OpenAI、本地模型等嵌入服务。
/// 切换模型只需更换实现，不影响上游代码。
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// 模型名称
    fn model_name(&self) -> &str;

    /// 嵌入维度
    fn dimension(&self) -> usize;

    /// 嵌入文本，返回向量
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// 批量嵌入文本
    async fn embed_texts(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_text(text).await?);
        }
        Ok(results)
    }

    /// 嵌入图像（预留接口）
    async fn embed_image(&self, _image_data: &[u8]) -> anyhow::Result<Vec<f32>> {
        anyhow::bail!("Image embedding not supported by {}", self.model_name())
    }
}

/// 知识存储接口
///
/// 定义知识条目的存储、查询和关联操作。
/// 底层可以是Qdrant+SQLite，也可以是纯内存实现。
#[async_trait]
pub trait KnowledgeStore: Send + Sync {
    /// 存储知识条目
    async fn store(&self, entry: KnowledgeEntry) -> anyhow::Result<String>;

    /// 语义搜索
    async fn query_semantic(
        &self,
        query: &str,
        embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>>;

    /// 关键词搜索
    async fn query_keyword(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>>;

    /// 建立知识关联
    async fn relate(&self, relation: KnowledgeRelation) -> anyhow::Result<()>;

    /// 获取关联知识
    async fn get_related(
        &self,
        entry_id: &str,
        depth: usize,
    ) -> anyhow::Result<Vec<KnowledgeRelation>>;

    /// 获取知识条目
    async fn get(&self, id: &str) -> anyhow::Result<Option<KnowledgeEntry>>;

    /// 删除知识条目
    async fn delete(&self, id: &str) -> anyhow::Result<()>;
}

/// 摄入管道
///
/// 将原始内容转化为可搜索的知识条目：
/// 解析 → 分块 → 嵌入 → 索引 → 关联
#[async_trait]
pub trait IngestionPipeline: Send + Sync {
    /// 摄入内容
    async fn ingest(&self, item: ContentItem) -> anyhow::Result<Vec<String>>;

    /// 批量摄入
    async fn ingest_batch(&self, items: Vec<ContentItem>) -> anyhow::Result<Vec<String>> {
        let mut ids = Vec::with_capacity(items.len());
        for item in items {
            ids.extend(self.ingest(item).await?);
        }
        Ok(ids)
    }
}
