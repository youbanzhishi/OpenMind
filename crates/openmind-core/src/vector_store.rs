//! 向量存储trait与内存实现
//!
//! 提供向量存储的抽象接口，支持可插拔的向量数据库：
//! - `VectorStore` trait: 向量存储核心接口
//! - `InMemoryVectorStore`: 内存向量存储（开发/测试用）
//! - `VectorStoreRegistry`: 向量存储注册表
//!
//! 生产环境可替换为Qdrant/Milvus/Weaviate等实现，通过registry注册即可。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// 向量点——向量数据库中的基本存储单元
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorPoint {
    /// 唯一标识
    pub id: String,
    /// 向量数据
    pub vector: Vec<f32>,
    /// 关联的元数据（如知识条目ID、来源等）
    pub metadata: HashMap<String, String>,
}

/// 向量搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// 匹配的向量点ID
    pub id: String,
    /// 相似度分数（0.0-1.0）
    pub score: f64,
    /// 关联元数据
    pub metadata: HashMap<String, String>,
}

/// 向量存储trait
///
/// 定义向量存储的核心接口。所有向量数据库（Qdrant/Milvus/内存等）
/// 实现此trait即可被系统使用，无需修改核心代码。
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// 存储后端名称
    fn name(&self) -> &str;

    /// 获取向量维度
    fn dimension(&self) -> usize;

    /// 插入或更新向量
    async fn upsert(&self, point: VectorPoint) -> anyhow::Result<()>;

    /// 批量插入或更新
    async fn upsert_batch(&self, points: Vec<VectorPoint>) -> anyhow::Result<()> {
        for point in points {
            self.upsert(point).await?;
        }
        Ok(())
    }

    /// 删除向量
    async fn delete(&self, id: &str) -> anyhow::Result<()>;

    /// 向量相似度搜索
    async fn search(
        &self,
        query: Vec<f32>,
        limit: usize,
        threshold: f64,
    ) -> anyhow::Result<Vec<VectorSearchResult>>;

    /// 根据ID获取向量
    async fn get(&self, id: &str) -> anyhow::Result<Option<VectorPoint>>;

    /// 获取存储的向量总数
    async fn count(&self) -> anyhow::Result<u64>;

    /// 健康检查
    async fn health_check(&self) -> bool {
        true
    }
}

/// 向量存储注册表
///
/// 管理多个向量存储实现，支持运行时按名称选择。
/// 新的向量存储只需调用register()注册即可。
pub struct VectorStoreRegistry {
    stores: HashMap<String, Box<dyn VectorStore>>,
    default: Option<String>,
}

impl VectorStoreRegistry {
    /// 创建空的注册表
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
            default: None,
        }
    }

    /// 注册向量存储
    pub fn register(&mut self, store: Box<dyn VectorStore>) {
        let name = store.name().to_string();
        if self.default.is_none() {
            self.default = Some(name.clone());
        }
        self.stores.insert(name, store);
    }

    /// 设置默认向量存储
    pub fn set_default(&mut self, name: &str) -> anyhow::Result<()> {
        if self.stores.contains_key(name) {
            self.default = Some(name.to_string());
            Ok(())
        } else {
            anyhow::bail!("Vector store '{}' not found in registry", name)
        }
    }

    /// 获取默认向量存储
    pub fn default_store(&self) -> Option<&dyn VectorStore> {
        self.default
            .as_ref()
            .and_then(|name| self.stores.get(name))
            .map(|s| s.as_ref())
    }

    /// 按名称获取向量存储
    pub fn get(&self, name: &str) -> Option<&dyn VectorStore> {
        self.stores.get(name).map(|s| s.as_ref())
    }

    /// 列出所有已注册的向量存储名称
    pub fn list_names(&self) -> Vec<String> {
        self.stores.keys().cloned().collect()
    }
}

impl Default for VectorStoreRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 内存向量存储
///
/// 纯内存实现，适用于开发测试。生产环境应替换为Qdrant等持久化方案。
/// 使用余弦相似度进行搜索。
pub struct InMemoryVectorStore {
    dimension: usize,
    points: Mutex<HashMap<String, VectorPoint>>,
}

impl InMemoryVectorStore {
    /// 创建指定维度的内存向量存储
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            points: Mutex::new(HashMap::new()),
        }
    }
}

/// 计算余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    fn name(&self) -> &str {
        "in_memory"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn upsert(&self, point: VectorPoint) -> anyhow::Result<()> {
        if point.vector.len() != self.dimension {
            anyhow::bail!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                point.vector.len()
            );
        }
        let mut points = self.points.lock().unwrap();
        points.insert(point.id.clone(), point);
        Ok(())
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        let mut points = self.points.lock().unwrap();
        points.remove(id);
        Ok(())
    }

    async fn search(
        &self,
        query: Vec<f32>,
        limit: usize,
        threshold: f64,
    ) -> anyhow::Result<Vec<VectorSearchResult>> {
        if query.len() != self.dimension {
            anyhow::bail!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimension,
                query.len()
            );
        }
        let points = self.points.lock().unwrap();
        let mut results: Vec<VectorSearchResult> = points
            .values()
            .filter_map(|p| {
                let score = cosine_similarity(&query, &p.vector);
                if score >= threshold {
                    Some(VectorSearchResult {
                        id: p.id.clone(),
                        score,
                        metadata: p.metadata.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results)
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<VectorPoint>> {
        let points = self.points.lock().unwrap();
        Ok(points.get(id).cloned())
    }

    async fn count(&self) -> anyhow::Result<u64> {
        let points = self.points.lock().unwrap();
        Ok(points.len() as u64)
    }

    async fn health_check(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_vector_store_basic() {
        let store = InMemoryVectorStore::new(4);

        let point = VectorPoint {
            id: "test-1".to_string(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: HashMap::from([("entry_id".to_string(), "entry-1".to_string())]),
        };

        store.upsert(point).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        let got = store.get("test-1").await.unwrap().unwrap();
        assert_eq!(got.id, "test-1");
    }

    #[tokio::test]
    async fn test_in_memory_vector_search() {
        let store = InMemoryVectorStore::new(4);

        // Insert vectors
        let points = vec![
            VectorPoint {
                id: "doc-1".to_string(),
                vector: vec![1.0, 0.0, 0.0, 0.0],
                metadata: HashMap::new(),
            },
            VectorPoint {
                id: "doc-2".to_string(),
                vector: vec![0.0, 1.0, 0.0, 0.0],
                metadata: HashMap::new(),
            },
            VectorPoint {
                id: "doc-3".to_string(),
                vector: vec![0.9, 0.1, 0.0, 0.0],
                metadata: HashMap::new(),
            },
        ];
        store.upsert_batch(points).await.unwrap();

        // Search for similar to [1,0,0,0]
        let results = store.search(vec![1.0, 0.0, 0.0, 0.0], 10, 0.5).await.unwrap();
        assert!(results.len() >= 2, "Should find at least 2 similar vectors");
        assert_eq!(results[0].id, "doc-1", "Most similar should be doc-1");
        assert!(results[0].score > 0.99, "doc-1 should have near-perfect score");
    }

    #[tokio::test]
    async fn test_in_memory_vector_delete() {
        let store = InMemoryVectorStore::new(4);

        let point = VectorPoint {
            id: "to-delete".to_string(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: HashMap::new(),
        };

        store.upsert(point).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        store.delete("to-delete").await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_vector_store_registry() {
        let mut registry = VectorStoreRegistry::new();
        let store = InMemoryVectorStore::new(128);
        registry.register(Box::new(store));

        assert_eq!(registry.list_names(), vec!["in_memory"]);
        assert!(registry.default_store().is_some());
        assert!(registry.get("in_memory").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_dimension_mismatch() {
        let store = InMemoryVectorStore::new(4);
        let point = VectorPoint {
            id: "bad".to_string(),
            vector: vec![1.0, 0.0], // wrong dimension
            metadata: HashMap::new(),
        };
        assert!(store.upsert(point).await.is_err());
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors
        let sim = cosine_similarity(&[1.0, 0.0, 0.0], &[1.0, 0.0, 0.0]);
        assert!((sim - 1.0).abs() < 0.001);

        // Orthogonal vectors
        let sim = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!((sim - 0.0).abs() < 0.001);

        // Opposite vectors
        let sim = cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]);
        assert!((sim - (-1.0)).abs() < 0.001);
    }
}
