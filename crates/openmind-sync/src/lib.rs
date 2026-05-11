//! OpenMind Sync - 同步调度与增量同步
//!
//! Phase 6: 同步调度器（定时/增量）、配置管理（热重载TOML）
//! Phase 7: 哈希变更检测、增量同步、删除处理、冲突解决、同步状态持久化

pub mod scheduler;
pub mod change_detector;
pub mod incremental;
pub mod conflict;
pub mod config;
pub mod monitor;

pub use scheduler::{SyncScheduler, SyncTask, SyncStrategy};
pub use change_detector::ChangeDetector;
pub use incremental::{IncrementalSync, SyncPlan};
pub use conflict::{ConflictResolver, ConflictStrategy, ConflictResult};
pub use config::{SyncConfig, SyncConfigManager};
pub use monitor::{SyncMonitor, SyncStatus, SyncMetrics, HealthStatus};
