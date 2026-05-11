//! API指标追踪
//!
//! 记录API调用次数、响应时间、错误率等监控指标。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// API指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiMetrics {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 按端点统计
    pub by_endpoint: HashMap<String, EndpointMetrics>,
    /// 按HTTP方法统计
    pub by_method: HashMap<String, u64>,
    /// 总响应时间(ms)
    pub total_response_time_ms: u64,
    /// 服务启动时间
    pub started_at: String,
}

/// 单个端点的指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EndpointMetrics {
    /// 请求数
    pub request_count: u64,
    /// 成功数
    pub success_count: u64,
    /// 失败数
    pub fail_count: u64,
    /// 总响应时间(ms)
    pub total_response_time_ms: u64,
    /// 最后一次请求时间
    pub last_request_at: Option<String>,
    /// 最后错误信息
    pub last_error: Option<String>,
}

/// 存储占用信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageInfo {
    /// SQLite数据库大小(bytes)
    pub db_size_bytes: u64,
    /// 总条目数
    pub total_entries: i64,
    /// 总关联数
    pub total_relations: i64,
    /// 嵌入状态统计
    pub embedding_stats: HashMap<String, i64>,
    /// 按来源统计
    pub source_stats: HashMap<String, i64>,
}

/// 监控仪表盘数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    /// API指标
    pub api: ApiMetrics,
    /// 存储信息
    pub storage: StorageInfo,
    /// 同步状态
    pub sync_status: Option<openmind_sync::SyncStatus>,
    /// 同步指标
    pub sync_metrics: Option<openmind_sync::SyncMetrics>,
    /// 同步健康
    pub sync_health: Option<String>,
    /// Connector列表
    pub connectors: Vec<String>,
    /// 嵌入模型状态
    pub embedding_available: bool,
}

/// API指标追踪器
pub struct ApiMetricsTracker {
    metrics: Mutex<ApiMetrics>,
}

impl ApiMetricsTracker {
    pub fn new() -> Self {
        Self {
            metrics: Mutex::new(ApiMetrics {
                started_at: chrono::Utc::now().to_rfc3339(),
                ..Default::default()
            }),
        }
    }

    /// 记录请求开始，返回guard用于记录结果
    pub fn start_request<'a>(&'a self, endpoint: &str, method: &str) -> RequestGuard<'a> {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.total_requests += 1;
            *metrics.by_method.entry(method.to_string()).or_insert(0) += 1;
            let em = metrics.by_endpoint.entry(endpoint.to_string()).or_default();
            em.request_count += 1;
            em.last_request_at = Some(chrono::Utc::now().to_rfc3339());
        }
        RequestGuard {
            tracker: self,
            endpoint: endpoint.to_string(),
            start: Instant::now(),
            finished: false,
        }
    }

    /// 获取指标快照
    pub fn get_metrics(&self) -> ApiMetrics {
        self.metrics.lock().unwrap().clone()
    }

    fn record_success(&self, endpoint: &str, duration_ms: u64) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.successful_requests += 1;
        metrics.total_response_time_ms += duration_ms;
        if let Some(em) = metrics.by_endpoint.get_mut(endpoint) {
            em.success_count += 1;
            em.total_response_time_ms += duration_ms;
        }
    }

    fn record_failure(&self, endpoint: &str, duration_ms: u64, error: &str) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.failed_requests += 1;
        metrics.total_response_time_ms += duration_ms;
        if let Some(em) = metrics.by_endpoint.get_mut(endpoint) {
            em.fail_count += 1;
            em.total_response_time_ms += duration_ms;
            em.last_error = Some(error.to_string());
        }
    }
}

impl Default for ApiMetricsTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// 请求守卫，drop时自动记录（如果未手动调用ok/err）
pub struct RequestGuard<'a> {
    tracker: &'a ApiMetricsTracker,
    endpoint: String,
    start: Instant,
    finished: bool,
}

impl<'a> RequestGuard<'a> {
    /// 标记请求成功
    pub fn ok(mut self) {
        let ms = self.start.elapsed().as_millis() as u64;
        self.tracker.record_success(&self.endpoint, ms);
        self.finished = true;
    }

    /// 标记请求失败
    pub fn err(mut self, error: &str) {
        let ms = self.start.elapsed().as_millis() as u64;
        self.tracker.record_failure(&self.endpoint, ms, error);
        self.finished = true;
    }
}

impl<'a> Drop for RequestGuard<'a> {
    fn drop(&mut self) {
        if !self.finished {
            let ms = self.start.elapsed().as_millis() as u64;
            self.tracker.record_success(&self.endpoint, ms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_tracker_record() {
        let tracker = ApiMetricsTracker::new();
        let guard = tracker.start_request("/api/v1/search", "POST");
        guard.ok();

        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.successful_requests, 1);
        assert_eq!(metrics.failed_requests, 0);
        assert!(metrics.by_endpoint.contains_key("/api/v1/search"));
        assert_eq!(metrics.by_endpoint["/api/v1/search"].success_count, 1);
    }

    #[test]
    fn test_metrics_tracker_failure() {
        let tracker = ApiMetricsTracker::new();
        let guard = tracker.start_request("/api/v1/ingest", "POST");
        guard.err("Internal error");

        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.failed_requests, 1);
        assert_eq!(metrics.by_endpoint["/api/v1/ingest"].fail_count, 1);
        assert_eq!(
            metrics.by_endpoint["/api/v1/ingest"].last_error,
            Some("Internal error".to_string())
        );
    }

    #[test]
    fn test_metrics_by_method() {
        let tracker = ApiMetricsTracker::new();
        let guard1 = tracker.start_request("/api/v1/search", "POST");
        guard1.ok();
        let guard2 = tracker.start_request("/api/v1/health", "GET");
        guard2.ok();

        let metrics = tracker.get_metrics();
        assert_eq!(*metrics.by_method.get("POST").unwrap_or(&0), 1);
        assert_eq!(*metrics.by_method.get("GET").unwrap_or(&0), 1);
    }

    #[test]
    fn test_guard_auto_ok_on_drop() {
        let tracker = ApiMetricsTracker::new();
        {
            let _guard = tracker.start_request("/api/v1/health", "GET");
            // guard dropped without explicit ok/err
        }
        let metrics = tracker.get_metrics();
        assert_eq!(metrics.successful_requests, 1);
    }
}
