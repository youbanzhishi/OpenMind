//! OpenMind API - HTTP API层
//!
//! 基于Axum的HTTP API，提供搜索、摄入、同步等REST接口，
//! 以及Agent发现协议（/.well-known/agent.json）。
//!
//! Phase 6: Web管理界面（HTMX+Alpine.js）、API指标追踪、配置热重载。

pub mod api_metrics;
pub mod routes;
pub mod state;
pub mod web_ui;

pub use api_metrics::{ApiMetrics, ApiMetricsTracker, DashboardData, StorageInfo};
pub use routes::create_router;
pub use state::AppState;
