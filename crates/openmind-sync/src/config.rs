//! 配置管理
//!
//! TOML配置文件，支持热重载。
//! 管理同步策略、调度间隔、冲突解决策略等。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// 同步配置（顶层）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// 调度配置
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    /// 各Connector的同步配置
    #[serde(default)]
    pub connectors: HashMap<String, ConnectorSyncConfig>,
    /// 冲突解决策略
    #[serde(default = "default_conflict_strategy")]
    pub conflict_strategy: String,
    /// 是否启用增量同步
    #[serde(default = "default_true_val")]
    pub incremental: bool,
    /// 内容哈希算法
    #[serde(default = "default_hash_algorithm")]
    pub hash_algorithm: String,
    /// 同步批大小
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// 删除处理模式
    #[serde(default = "default_delete_mode")]
    pub delete_mode: String,
}

fn default_conflict_strategy() -> String { "last_write_wins".to_string() }
fn default_true_val() -> bool { true }
fn default_hash_algorithm() -> String { "sha256".to_string() }
fn default_batch_size() -> usize { 100 }
fn default_delete_mode() -> String { "cascade".to_string() }
fn default_true() -> bool { true }
fn default_incremental() -> String { "incremental".to_string() }

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            scheduler: SchedulerConfig::default(),
            connectors: HashMap::new(),
            conflict_strategy: default_conflict_strategy(),
            incremental: true,
            hash_algorithm: default_hash_algorithm(),
            batch_size: default_batch_size(),
            delete_mode: default_delete_mode(),
        }
    }
}

/// 调度器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// 是否启用定时同步
    #[serde(default = "default_true_val")]
    pub enabled: bool,
    /// 默认同步间隔（秒）
    #[serde(default = "default_interval")]
    pub default_interval_secs: u64,
    /// 最大并发同步任务数
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    /// 同步失败重试次数
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
    /// 重试间隔（秒）
    #[serde(default = "default_retry_interval")]
    pub retry_interval_secs: u64,
}

fn default_interval() -> u64 { 300 }
fn default_max_concurrent() -> usize { 3 }
fn default_retry_count() -> u32 { 3 }
fn default_retry_interval() -> u64 { 30 }

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_interval_secs: default_interval(),
            max_concurrent: default_max_concurrent(),
            retry_count: default_retry_count(),
            retry_interval_secs: default_retry_interval(),
        }
    }
}

/// 单个Connector的同步配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorSyncConfig {
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 同步间隔（秒），0=使用默认
    #[serde(default)]
    pub interval_secs: u64,
    /// 同步策略：full / incremental
    #[serde(default = "default_incremental")]
    pub strategy: String,
    /// 冲突解决策略覆盖
    #[serde(default)]
    pub conflict_strategy: Option<String>,
    /// 额外参数
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

impl Default for ConnectorSyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 0,
            strategy: default_incremental(),
            conflict_strategy: None,
            params: HashMap::new(),
        }
    }
}

/// 配置管理器（支持热重载）
pub struct SyncConfigManager {
    config: Mutex<SyncConfig>,
    config_path: Option<String>,
}

impl SyncConfigManager {
    /// 从默认配置创建
    pub fn new() -> Self {
        Self {
            config: Mutex::new(SyncConfig::default()),
            config_path: None,
        }
    }

    /// 从TOML文件加载
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: SyncConfig = toml::from_str(&content)?;
        Ok(Self {
            config: Mutex::new(config),
            config_path: Some(path.to_string()),
        })
    }

    /// 从TOML字符串解析
    pub fn from_toml(toml_str: &str) -> anyhow::Result<Self> {
        let config: SyncConfig = toml::from_str(toml_str)?;
        Ok(Self {
            config: Mutex::new(config),
            config_path: None,
        })
    }

    /// 获取当前配置
    pub fn get_config(&self) -> SyncConfig {
        self.config.lock().unwrap().clone()
    }

    /// 热重载配置（从文件重新读取）
    pub fn reload(&self) -> anyhow::Result<bool> {
        if let Some(ref path) = self.config_path {
            let content = std::fs::read_to_string(path)?;
            let new_config: SyncConfig = toml::from_str(&content)?;
            let mut config = self.config.lock().unwrap();
            *config = new_config;
            tracing::info!("Config reloaded from {}", path);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 更新配置
    pub fn update(&self, new_config: SyncConfig) {
        let mut config = self.config.lock().unwrap();
        *config = new_config;
    }

    /// 获取指定Connector的配置
    pub fn get_connector_config(&self, name: &str) -> ConnectorSyncConfig {
        let config = self.config.lock().unwrap();
        config.connectors.get(name).cloned().unwrap_or_default()
    }

    /// 导出为TOML字符串
    pub fn to_toml(&self) -> anyhow::Result<String> {
        let config = self.config.lock().unwrap();
        Ok(toml::to_string_pretty(&*config)?)
    }
}

impl Default for SyncConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SyncConfig::default();
        assert!(config.incremental);
        assert_eq!(config.conflict_strategy, "last_write_wins");
        assert_eq!(config.batch_size, 100);
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
conflict_strategy = "manual"
incremental = true
batch_size = 50
delete_mode = "soft"

[scheduler]
enabled = true
default_interval_secs = 600
max_concurrent = 5

[connectors.vault]
enabled = true
interval_secs = 120
strategy = "incremental"

[connectors.blog]
enabled = true
strategy = "full"
"#;
        let manager = SyncConfigManager::from_toml(toml_str).unwrap();
        let config = manager.get_config();
        assert_eq!(config.scheduler.default_interval_secs, 600);
        assert_eq!(config.conflict_strategy, "manual");
        assert_eq!(config.batch_size, 50);
        assert!(config.connectors.contains_key("vault"));
        assert_eq!(config.connectors["vault"].interval_secs, 120);
    }

    #[test]
    fn test_config_manager_update() {
        let manager = SyncConfigManager::new();
        let mut new_config = SyncConfig::default();
        new_config.batch_size = 200;
        manager.update(new_config);
        assert_eq!(manager.get_config().batch_size, 200);
    }

    #[test]
    fn test_config_roundtrip() {
        let config = SyncConfig::default();
        let manager = SyncConfigManager::new();
        manager.update(config.clone());
        let toml_str = manager.to_toml().unwrap();
        let manager2 = SyncConfigManager::from_toml(&toml_str).unwrap();
        let config2 = manager2.get_config();
        assert_eq!(config.batch_size, config2.batch_size);
        assert_eq!(config.conflict_strategy, config2.conflict_strategy);
    }

    #[test]
    fn test_connector_config_default() {
        let manager = SyncConfigManager::new();
        let cc = manager.get_connector_config("nonexistent");
        assert!(cc.enabled);
        assert_eq!(cc.strategy, "incremental");
    }
}
