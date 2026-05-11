//! Connector注册表与增强trait
//!
//! 提供Connector的注册、发现和健康检查机制。
//! 每个Connector声明自己的能力，系统自动发现和编排。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::registry::Capability;
use crate::traits::Connector;

/// Connector能力声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorCapabilities {
    /// 支持的内容格式
    pub supported_formats: Vec<String>,
    /// 轮询策略（poll/event/webhook）
    pub poll_strategy: String,
    /// 同步模式（full/incremental）
    pub sync_mode: String,
    /// 额外能力
    pub extra_capabilities: Vec<Capability>,
}

impl ConnectorCapabilities {
    /// 创建新的能力声明
    pub fn new(supported_formats: Vec<&str>, poll_strategy: &str, sync_mode: &str) -> Self {
        Self {
            supported_formats: supported_formats.into_iter().map(String::from).collect(),
            poll_strategy: poll_strategy.to_string(),
            sync_mode: sync_mode.to_string(),
            extra_capabilities: Vec::new(),
        }
    }

    /// 添加额外能力
    pub fn with_capability(mut self, cap: Capability) -> Self {
        self.extra_capabilities.push(cap);
        self
    }
}

/// Connector trait增强
///
/// 在基础Connector trait之上，增加能力声明和健康检查。
#[async_trait]
pub trait EnhancedConnector: Connector {
    /// 获取Connector的能力声明
    fn capabilities(&self) -> ConnectorCapabilities;

    /// 健康检查
    async fn health_check(&self) -> ConnectorHealth {
        ConnectorHealth {
            name: self.name().to_string(),
            is_healthy: true,
            message: "OK".to_string(),
            last_checked: chrono::Utc::now(),
        }
    }
}

/// Connector健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorHealth {
    /// Connector名称
    pub name: String,
    /// 是否健康
    pub is_healthy: bool,
    /// 状态消息
    pub message: String,
    /// 最后检查时间
    pub last_checked: chrono::DateTime<chrono::Utc>,
}

/// Connector注册表
///
/// 管理所有Connector实例，支持注册、发现和健康检查。
/// 新Connector只需调用register()即可被系统使用。
pub struct ConnectorRegistry {
    connectors: Mutex<HashMap<String, Box<dyn EnhancedConnector>>>,
}

impl ConnectorRegistry {
    /// 创建空的注册表
    pub fn new() -> Self {
        Self {
            connectors: Mutex::new(HashMap::new()),
        }
    }

    /// 注册Connector
    pub fn register(&self, connector: Box<dyn EnhancedConnector>) {
        let mut connectors = self.connectors.lock().unwrap();
        let name = connector.name().to_string();
        connectors.insert(name, connector);
    }

    /// 获取Connector
    pub fn get(
        &self,
        _name: &str,
    ) -> Option<std::sync::MutexGuard<'_, HashMap<String, Box<dyn EnhancedConnector>>>> {
        // Can't return the guard with a reference, need different approach
        None
    }

    /// 按名称获取Connector引用（执行闭包）
    pub fn with_connector<F, R>(&self, name: &str, f: F) -> Option<R>
    where
        F: FnOnce(&dyn EnhancedConnector) -> R,
    {
        let connectors = self.connectors.lock().unwrap();
        connectors.get(name).map(|c| f(c.as_ref()))
    }

    /// 列出所有已注册Connector的名称
    pub fn list_names(&self) -> Vec<String> {
        let connectors = self.connectors.lock().unwrap();
        connectors.keys().cloned().collect()
    }

    /// 获取所有Connector的能力
    pub fn list_capabilities(&self) -> Vec<(String, ConnectorCapabilities)> {
        let connectors = self.connectors.lock().unwrap();
        connectors
            .values()
            .map(|c| (c.name().to_string(), c.capabilities()))
            .collect()
    }

    /// 按能力搜索Connector
    pub fn find_by_format(&self, format: &str) -> Vec<String> {
        let connectors = self.connectors.lock().unwrap();
        connectors
            .values()
            .filter(|c| {
                c.capabilities()
                    .supported_formats
                    .contains(&format.to_string())
            })
            .map(|c| c.name().to_string())
            .collect()
    }

    /// 批量健康检查
    pub async fn health_check_all(&self) -> Vec<ConnectorHealth> {
        let connectors = self.connectors.lock().unwrap();
        let mut results = Vec::new();
        for connector in connectors.values() {
            let health = connector.health_check().await;
            results.push(health);
        }
        results
    }

    /// 初始化所有Connector（建立连接）
    pub async fn connect_all(&self) -> Vec<anyhow::Result<()>> {
        let connectors = self.connectors.lock().unwrap();
        let mut results = Vec::new();
        for connector in connectors.values() {
            let result = connector.connect().await;
            results.push(result);
        }
        results
    }
}

impl Default for ConnectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContentChange, ContentItem, SyncState};
    use crate::traits::Connector;

    struct DummyConnector;

    #[async_trait]
    impl Connector for DummyConnector {
        fn name(&self) -> &str {
            "dummy"
        }
        async fn connect(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn list_changes(&self, _since: &SyncState) -> anyhow::Result<Vec<ContentChange>> {
            Ok(vec![])
        }
        async fn fetch_content(&self, _id: &str) -> anyhow::Result<ContentItem> {
            anyhow::bail!("Not implemented")
        }
    }

    #[async_trait]
    impl EnhancedConnector for DummyConnector {
        fn capabilities(&self) -> ConnectorCapabilities {
            ConnectorCapabilities::new(vec!["text", "markdown"], "poll", "full")
        }
    }

    #[test]
    fn test_connector_registry() {
        let registry = ConnectorRegistry::new();
        registry.register(Box::new(DummyConnector));

        assert_eq!(registry.list_names(), vec!["dummy"]);

        let result =
            registry.with_connector("dummy", |c| c.capabilities().supported_formats.clone());
        assert_eq!(
            result,
            Some(vec!["text".to_string(), "markdown".to_string()])
        );
    }

    #[test]
    fn test_find_by_format() {
        let registry = ConnectorRegistry::new();
        registry.register(Box::new(DummyConnector));

        let found = registry.find_by_format("markdown");
        assert_eq!(found, vec!["dummy"]);

        let not_found = registry.find_by_format("pdf");
        assert!(not_found.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_all() {
        let registry = ConnectorRegistry::new();
        registry.register(Box::new(DummyConnector));

        let health = registry.health_check_all().await;
        assert_eq!(health.len(), 1);
        assert!(health[0].is_healthy);
    }
}
