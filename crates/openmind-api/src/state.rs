//! 应用状态

use openmind_core::SqliteKnowledgeStore;
use std::sync::Arc;

/// 应用共享状态
///
/// 通过Axum的State机制注入到各路由处理器。
pub struct AppState {
    /// 服务版本
    pub version: String,
    /// 已注册的Connector列表
    pub connectors: Vec<String>,
    /// 知识存储
    pub store: Arc<SqliteKnowledgeStore>,
    /// 嵌入模型状态
    pub embedding_available: bool,
}

impl AppState {
    pub fn new(store: SqliteKnowledgeStore) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            connectors: Vec::new(),
            store: Arc::new(store),
            embedding_available: true,
        }
    }

    pub fn with_embedding(mut self, available: bool) -> Self {
        self.embedding_available = available;
        self
    }
}
