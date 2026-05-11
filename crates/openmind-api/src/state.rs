//! 应用状态

use openmind_actions::ActionRegistry;
use openmind_core::SqliteKnowledgeStore;
use openmind_sync::{SyncConfigManager, SyncMonitor, SyncScheduler};
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
    /// Action注册表 (Phase 5)
    pub action_registry: Option<Arc<ActionRegistry>>,
    /// 同步调度器 (Phase 6)
    pub sync_scheduler: Option<Arc<SyncScheduler>>,
    /// 同步监控 (Phase 6)
    pub sync_monitor: Option<Arc<SyncMonitor>>,
    /// 同步配置管理 (Phase 6)
    pub sync_config: Option<Arc<SyncConfigManager>>,
}

impl AppState {
    pub fn new(store: SqliteKnowledgeStore) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            connectors: Vec::new(),
            store: Arc::new(store),
            embedding_available: true,
            action_registry: None,
            sync_scheduler: None,
            sync_monitor: None,
            sync_config: None,
        }
    }

    pub fn with_embedding(mut self, available: bool) -> Self {
        self.embedding_available = available;
        self
    }

    pub fn with_action_registry(mut self, registry: Arc<ActionRegistry>) -> Self {
        self.action_registry = Some(registry);
        self
    }

    pub fn with_sync_scheduler(mut self, scheduler: Arc<SyncScheduler>) -> Self {
        self.sync_scheduler = Some(scheduler);
        self
    }

    pub fn with_sync_monitor(mut self, monitor: Arc<SyncMonitor>) -> Self {
        self.sync_monitor = Some(monitor);
        self
    }

    pub fn with_sync_config(mut self, config: Arc<SyncConfigManager>) -> Self {
        self.sync_config = Some(config);
        self
    }
}
