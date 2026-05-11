//! 监控面板
//!
//! 统计/状态/健康监控。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// 同步状态概览
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// 各Connector的最后同步时间
    pub last_sync: HashMap<String, String>,
    /// 各Connector的同步状态
    pub connector_status: HashMap<String, String>,
    /// 活跃同步任务数
    pub active_tasks: usize,
    /// 调度器是否运行
    pub scheduler_running: bool,
}

/// 同步指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncMetrics {
    /// 总同步次数
    pub total_syncs: u64,
    /// 成功次数
    pub successful_syncs: u64,
    /// 失败次数
    pub failed_syncs: u64,
    /// 总同步条目数
    pub total_items_synced: u64,
    /// 总冲突数
    pub total_conflicts: u64,
    /// 各Connector的同步计数
    pub by_connector: HashMap<String, ConnectorMetrics>,
}

/// 单个Connector的指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectorMetrics {
    pub sync_count: u64,
    pub success_count: u64,
    pub fail_count: u64,
    pub items_synced: u64,
    pub conflicts: u64,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
}

/// 同步监控器
pub struct SyncMonitor {
    metrics: Mutex<SyncMetrics>,
    last_sync: Mutex<HashMap<String, String>>,
    scheduler_running: Mutex<bool>,
}

impl SyncMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Mutex::new(SyncMetrics::default()),
            last_sync: Mutex::new(HashMap::new()),
            scheduler_running: Mutex::new(false),
        }
    }

    /// 记录同步成功
    pub fn record_sync_success(&self, connector: &str, items: u64) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.total_syncs += 1;
        metrics.successful_syncs += 1;
        metrics.total_items_synced += items;

        let cm = metrics
            .by_connector
            .entry(connector.to_string())
            .or_default();
        cm.sync_count += 1;
        cm.success_count += 1;
        cm.items_synced += items;
        cm.last_sync_at = Some(chrono::Utc::now().to_rfc3339());

        let mut last_sync = self.last_sync.lock().unwrap();
        last_sync.insert(connector.to_string(), chrono::Utc::now().to_rfc3339());
    }

    /// 记录同步失败
    pub fn record_sync_failure(&self, connector: &str, error: &str) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.total_syncs += 1;
        metrics.failed_syncs += 1;

        let cm = metrics
            .by_connector
            .entry(connector.to_string())
            .or_default();
        cm.sync_count += 1;
        cm.fail_count += 1;
        cm.last_error = Some(error.to_string());
    }

    /// 记录冲突
    pub fn record_conflict(&self, connector: &str) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.total_conflicts += 1;

        let cm = metrics
            .by_connector
            .entry(connector.to_string())
            .or_default();
        cm.conflicts += 1;
    }

    /// 获取指标
    pub fn get_metrics(&self) -> SyncMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// 获取状态概览
    pub fn get_status(&self, active_tasks: usize) -> SyncStatus {
        let last_sync = self.last_sync.lock().unwrap().clone();
        let metrics = self.metrics.lock().unwrap();
        let scheduler_running = *self.scheduler_running.lock().unwrap();

        let connector_status: HashMap<String, String> = metrics
            .by_connector
            .iter()
            .map(|(name, m)| {
                let status = if m.fail_count > 0 && m.success_count == 0 {
                    "error".to_string()
                } else if m.last_sync_at.is_some() {
                    "healthy".to_string()
                } else {
                    "never_synced".to_string()
                };
                (name.clone(), status)
            })
            .collect();

        SyncStatus {
            last_sync,
            connector_status,
            active_tasks,
            scheduler_running,
        }
    }

    /// 设置调度器运行状态
    pub fn set_scheduler_running(&self, running: bool) {
        let mut sr = self.scheduler_running.lock().unwrap();
        *sr = running;
    }

    /// 健康检查
    pub fn health_check(&self) -> HealthStatus {
        let metrics = self.metrics.lock().unwrap();
        let success_rate = if metrics.total_syncs > 0 {
            metrics.successful_syncs as f64 / metrics.total_syncs as f64
        } else {
            1.0
        };

        if success_rate >= 0.9 {
            HealthStatus::Healthy
        } else if success_rate >= 0.5 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        }
    }
}

impl Default for SyncMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// 健康状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_record_success() {
        let monitor = SyncMonitor::new();
        monitor.record_sync_success("vault", 10);
        let metrics = monitor.get_metrics();
        assert_eq!(metrics.total_syncs, 1);
        assert_eq!(metrics.successful_syncs, 1);
        assert_eq!(metrics.total_items_synced, 10);
    }

    #[test]
    fn test_monitor_record_failure() {
        let monitor = SyncMonitor::new();
        monitor.record_sync_failure("vault", "Connection refused");
        let metrics = monitor.get_metrics();
        assert_eq!(metrics.total_syncs, 1);
        assert_eq!(metrics.failed_syncs, 1);
    }

    #[test]
    fn test_monitor_conflict() {
        let monitor = SyncMonitor::new();
        monitor.record_conflict("vault");
        let metrics = monitor.get_metrics();
        assert_eq!(metrics.total_conflicts, 1);
    }

    #[test]
    fn test_health_check_healthy() {
        let monitor = SyncMonitor::new();
        monitor.record_sync_success("vault", 5);
        assert_eq!(monitor.health_check(), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_check_unhealthy() {
        let monitor = SyncMonitor::new();
        monitor.record_sync_failure("vault", "error");
        monitor.record_sync_failure("vault", "error");
        assert_eq!(monitor.health_check(), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_get_status() {
        let monitor = SyncMonitor::new();
        monitor.record_sync_success("vault", 5);
        monitor.set_scheduler_running(true);
        let status = monitor.get_status(0);
        assert!(status.last_sync.contains_key("vault"));
        assert!(status.scheduler_running);
    }
}
