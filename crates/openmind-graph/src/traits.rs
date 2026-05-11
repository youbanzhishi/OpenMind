//! 图数据库隔离trait
//!
//! 通过trait隔离图数据库选型，支持可替换的图存储后端。
//! 默认提供InMemoryGraphStore，生产环境可替换为Neo4j等。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 图节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// 节点ID
    pub id: String,
    /// 节点标签
    pub label: String,
    /// 节点属性
    pub properties: serde_json::Value,
}

/// 图边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// 边ID
    pub id: String,
    /// 起始节点ID
    pub from_id: String,
    /// 目标节点ID
    pub to_id: String,
    /// 边类型
    pub edge_type: String,
    /// 权重
    pub weight: f64,
    /// 边属性
    pub properties: serde_json::Value,
}

/// 图搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSearchResult {
    /// 匹配的节点
    pub node: GraphNode,
    /// 关联的边
    pub edges: Vec<GraphEdge>,
    /// 相关度
    pub relevance: f64,
}

/// 图存储trait
///
/// 定义图数据库的核心接口，所有图数据库（Neo4j/内存等）
/// 实现此trait即可被系统使用。
#[async_trait]
pub trait GraphStore: Send + Sync {
    /// 存储名称
    fn name(&self) -> &str;

    /// 添加节点
    async fn add_node(&self, node: GraphNode) -> anyhow::Result<()>;

    /// 添加边
    async fn add_edge(&self, edge: GraphEdge) -> anyhow::Result<()>;

    /// 获取节点
    async fn get_node(&self, id: &str) -> anyhow::Result<Option<GraphNode>>;

    /// 获取节点的所有边
    async fn get_edges(&self, node_id: &str) -> anyhow::Result<Vec<GraphEdge>>;

    /// 搜索节点（按标签和属性）
    async fn search_nodes(
        &self,
        label: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<GraphSearchResult>>;

    /// 获取邻居节点（BFS遍历）
    async fn get_neighbors(
        &self,
        node_id: &str,
        depth: usize,
    ) -> anyhow::Result<Vec<GraphSearchResult>>;

    /// 删除节点
    async fn delete_node(&self, id: &str) -> anyhow::Result<()>;

    /// 删除边
    async fn delete_edge(&self, id: &str) -> anyhow::Result<()>;

    /// 统计节点数
    async fn count_nodes(&self) -> anyhow::Result<u64>;

    /// 统计边数
    async fn count_edges(&self) -> anyhow::Result<u64>;

    /// 健康检查
    async fn health_check(&self) -> bool {
        true
    }
}

/// 内存图存储
///
/// 纯内存实现，适用于开发测试。
pub struct InMemoryGraphStore {
    nodes: std::sync::Mutex<std::collections::HashMap<String, GraphNode>>,
    edges: std::sync::Mutex<std::collections::HashMap<String, GraphEdge>>,
}

impl InMemoryGraphStore {
    /// 创建空的内存图存储
    pub fn new() -> Self {
        Self {
            nodes: std::sync::Mutex::new(std::collections::HashMap::new()),
            edges: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GraphStore for InMemoryGraphStore {
    fn name(&self) -> &str {
        "in_memory"
    }

    async fn add_node(&self, node: GraphNode) -> anyhow::Result<()> {
        let mut nodes = self.nodes.lock().unwrap();
        nodes.insert(node.id.clone(), node);
        Ok(())
    }

    async fn add_edge(&self, edge: GraphEdge) -> anyhow::Result<()> {
        let mut edges = self.edges.lock().unwrap();
        edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    async fn get_node(&self, id: &str) -> anyhow::Result<Option<GraphNode>> {
        let nodes = self.nodes.lock().unwrap();
        Ok(nodes.get(id).cloned())
    }

    async fn get_edges(&self, node_id: &str) -> anyhow::Result<Vec<GraphEdge>> {
        let edges = self.edges.lock().unwrap();
        Ok(edges
            .values()
            .filter(|e| e.from_id == node_id || e.to_id == node_id)
            .cloned()
            .collect())
    }

    async fn search_nodes(
        &self,
        label: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<GraphSearchResult>> {
        let nodes = self.nodes.lock().unwrap();
        let edges = self.edges.lock().unwrap();

        let results: Vec<GraphSearchResult> = nodes
            .values()
            .filter(|n| {
                n.label == label
                    && (n.properties.to_string().to_lowercase().contains(&query.to_lowercase())
                        || n.id.contains(query))
            })
            .take(limit)
            .map(|n| {
                let node_edges: Vec<GraphEdge> = edges
                    .values()
                    .filter(|e| e.from_id == n.id || e.to_id == n.id)
                    .cloned()
                    .collect();
                GraphSearchResult {
                    node: n.clone(),
                    edges: node_edges,
                    relevance: 1.0,
                }
            })
            .collect();

        Ok(results)
    }

    async fn get_neighbors(
        &self,
        node_id: &str,
        depth: usize,
    ) -> anyhow::Result<Vec<GraphSearchResult>> {
        let nodes = self.nodes.lock().unwrap();
        let edges = self.edges.lock().unwrap();

        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        visited.insert(node_id.to_string());
        let mut current_ids = vec![node_id.to_string()];

        for _ in 0..depth {
            let mut next_ids = Vec::new();
            for id in &current_ids {
                for edge in edges.values() {
                    let neighbor_id = if edge.from_id == *id {
                        &edge.to_id
                    } else if edge.to_id == *id {
                        &edge.from_id
                    } else {
                        continue;
                    };

                    if !visited.contains(neighbor_id) {
                        visited.insert(neighbor_id.clone());
                        next_ids.push(neighbor_id.clone());

                        if let Some(node) = nodes.get(neighbor_id) {
                            let node_edges: Vec<GraphEdge> = edges
                                .values()
                                .filter(|e| e.from_id == *neighbor_id || e.to_id == *neighbor_id)
                                .cloned()
                                .collect();
                            results.push(GraphSearchResult {
                                node: node.clone(),
                                edges: node_edges,
                                relevance: 1.0 / (results.len() as f64 + 1.0),
                            });
                        }
                    }
                }
            }
            current_ids = next_ids;
        }

        Ok(results)
    }

    async fn delete_node(&self, id: &str) -> anyhow::Result<()> {
        let mut nodes = self.nodes.lock().unwrap();
        nodes.remove(id);
        Ok(())
    }

    async fn delete_edge(&self, id: &str) -> anyhow::Result<()> {
        let mut edges = self.edges.lock().unwrap();
        edges.remove(id);
        Ok(())
    }

    async fn count_nodes(&self) -> anyhow::Result<u64> {
        let nodes = self.nodes.lock().unwrap();
        Ok(nodes.len() as u64)
    }

    async fn count_edges(&self) -> anyhow::Result<u64> {
        let edges = self.edges.lock().unwrap();
        Ok(edges.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_graph_basic() {
        let store = InMemoryGraphStore::new();

        let node = GraphNode {
            id: "rust".to_string(),
            label: "technology".to_string(),
            properties: serde_json::json!({"name": "Rust", "type": "language"}),
        };
        store.add_node(node).await.unwrap();

        let got = store.get_node("rust").await.unwrap().unwrap();
        assert_eq!(got.label, "technology");
    }

    #[tokio::test]
    async fn test_in_memory_graph_edges() {
        let store = InMemoryGraphStore::new();

        store.add_node(GraphNode {
            id: "rust".to_string(),
            label: "tech".to_string(),
            properties: serde_json::json!({}),
        }).await.unwrap();

        store.add_node(GraphNode {
            id: "safety".to_string(),
            label: "concept".to_string(),
            properties: serde_json::json!({}),
        }).await.unwrap();

        store.add_edge(GraphEdge {
            id: "e1".to_string(),
            from_id: "rust".to_string(),
            to_id: "safety".to_string(),
            edge_type: "focuses_on".to_string(),
            weight: 0.9,
            properties: serde_json::json!({}),
        }).await.unwrap();

        let edges = store.get_edges("rust").await.unwrap();
        assert_eq!(edges.len(), 1);
    }

    #[tokio::test]
    async fn test_in_memory_graph_neighbors() {
        let store = InMemoryGraphStore::new();

        store.add_node(GraphNode { id: "a".to_string(), label: "test".to_string(), properties: serde_json::json!({}) }).await.unwrap();
        store.add_node(GraphNode { id: "b".to_string(), label: "test".to_string(), properties: serde_json::json!({}) }).await.unwrap();
        store.add_node(GraphNode { id: "c".to_string(), label: "test".to_string(), properties: serde_json::json!({}) }).await.unwrap();

        store.add_edge(GraphEdge { id: "e1".to_string(), from_id: "a".to_string(), to_id: "b".to_string(), edge_type: "related".to_string(), weight: 1.0, properties: serde_json::json!({}) }).await.unwrap();
        store.add_edge(GraphEdge { id: "e2".to_string(), from_id: "b".to_string(), to_id: "c".to_string(), edge_type: "related".to_string(), weight: 1.0, properties: serde_json::json!({}) }).await.unwrap();

        let neighbors = store.get_neighbors("a", 1).await.unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].node.id, "b");

        let neighbors = store.get_neighbors("a", 2).await.unwrap();
        assert_eq!(neighbors.len(), 2);
    }
}
