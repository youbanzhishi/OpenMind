//! 同步调度器
//!
//! 定时/增量同步调度，策略可配置。
//! 支持定时触发和手动触发同步任务。

use crate::config::SyncConfigManager;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// 同步策略
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStrategy {
    /// 全量同步
    Full,
    /// 增量同步
    Incremental,
    /// 定时同步
    Scheduled,
}

impl SyncStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "full" => Some(Self::Full),
            "incremental" => Some(Self::Incremental),
            "scheduled" => Some(Self::Scheduled),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Full => "full",
            Self::Incremental => "incremental",
            Self::Scheduled => "scheduled",
        }
    }
}

/// 同步任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTask {
    /// 任务ID
    pub id: String,
    /// Connector名称
    pub connector_name: String,
    /// 同步策略
    pub strategy: SyncStrategy,
    /// 任务状态
    pub status: SyncTaskStatus,
    /// 创建时间
    pub created_at: String,
    /// 完成时间
    pub completed_at: Option<String>,
    /// 结果摘要
    pub result_summary: Option<String>,
}

/// 同步任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// 同步调度器
pub struct SyncScheduler {
    /// 配置管理器
    config: SyncConfigManager,
    /// 活跃任务
    tasks: Mutex<Vec<SyncTask>>,
    /// 上次同步时间 (connector_name -> timestamp)
    last_sync: Mutex<HashMap<String, String>>,
    /// 是否运行中
    running: Mutex<bool>,
}

impl SyncScheduler {
    pub fn new(config: SyncConfigManager) -> Self {
        Self {
            config,
            tasks: Mutex::new(Vec::new()),
            last_sync: Mutex::new(HashMap::new()),
            running: Mutex::new(false),
        }
    }

    /// 创建同步任务
    pub fn create_task(&self, connector_name: &str, strategy: SyncStrategy) -> SyncTask {
        let task = SyncTask {
            id: uuid::Uuid::new_v4().to_string(),
            connector_name: connector_name.to_string(),
            strategy,
            status: SyncTaskStatus::Pending,
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            result_summary: None,
        };

        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(task.clone());
        task
    }

    /// 获取待执行的任务
    pub fn get_pending_tasks(&self) -> Vec<SyncTask> {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .iter()
            .filter(|t| t.status == SyncTaskStatus::Pending)
            .cloned()
            .collect()
    }

    /// 标记任务为运行中
    pub fn mark_running(&self, task_id: &str) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = SyncTaskStatus::Running;
        }
    }

    /// 完成任务
    pub fn mark_completed(&self, task_id: &str, summary: &str) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = SyncTaskStatus::Completed;
            task.completed_at = Some(chrono::Utc::now().to_rfc3339());
            task.result_summary = Some(summary.to_string());
        }

        // Update last sync time
        let mut last_sync = self.last_sync.lock().unwrap();
        if let Some(task) = tasks.iter().find(|t| t.id == task_id) {
            last_sync.insert(task.connector_name.clone(), chrono::Utc::now().to_rfc3339());
        }
    }

    /// 标记任务失败
    pub fn mark_failed(&self, task_id: &str, error: &str) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = SyncTaskStatus::Failed;
            task.completed_at = Some(chrono::Utc::now().to_rfc3339());
            task.result_summary = Some(error.to_string());
        }
    }

    /// 检查是否应该触发同步
    pub fn should_sync(&self, connector_name: &str) -> bool {
        let config = self.config.get_config();
        let cc = config.connectors.get(connector_name);

        let interval = cc
            .map(|c| {
                if c.interval_secs == 0 {
                    config.scheduler.default_interval_secs
                } else {
                    c.interval_secs
                }
            })
            .unwrap_or(config.scheduler.default_interval_secs);

        let last_sync = self.last_sync.lock().unwrap();
        match last_sync.get(connector_name) {
            None => true, // Never synced
            Some(last) => {
                if let Ok(last_time) = chrono::DateTime::parse_from_rfc3339(last) {
                    let elapsed = chrono::Utc::now() - last_time.with_timezone(&chrono::Utc);
                    elapsed.num_seconds() as u64 >= interval
                } else {
                    true
                }
            }
        }
    }

    /// 获取所有需要同步的Connector
    pub fn get_due_connectors(&self) -> Vec<String> {
        let config = self.config.get_config();
        config
            .connectors
            .keys()
            .filter(|name| self.should_sync(name))
            .cloned()
            .collect()
    }

    /// 获取任务历史
    pub fn get_task_history(&self, limit: usize) -> Vec<SyncTask> {
        let tasks = self.tasks.lock().unwrap();
        tasks.iter().rev().take(limit).cloned().collect()
    }

    /// 获取活跃任务数
    pub fn active_task_count(&self) -> usize {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .iter()
            .filter(|t| t.status == SyncTaskStatus::Running)
            .count()
    }

    /// 启动调度器
    pub fn start(&self) {
        let mut running = self.running.lock().unwrap();
        *running = true;
        tracing::info!("Sync scheduler started");
    }

    /// 停止调度器
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
        tracing::info!("Sync scheduler stopped");
    }

    /// 是否运行中
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_task() {
        let scheduler = SyncScheduler::new(SyncConfigManager::new());
        let task = scheduler.create_task("vault", SyncStrategy::Incremental);
        assert_eq!(task.connector_name, "vault");
        assert_eq!(task.status, SyncTaskStatus::Pending);
    }

    #[test]
    fn test_task_lifecycle() {
        let scheduler = SyncScheduler::new(SyncConfigManager::new());
        let task = scheduler.create_task("vault", SyncStrategy::Full);

        scheduler.mark_running(&task.id);
        let pending = scheduler.get_pending_tasks();
        assert!(pending.is_empty());

        scheduler.mark_completed(&task.id, "Synced 10 items");
        let history = scheduler.get_task_history(10);
        assert_eq!(history[0].status, SyncTaskStatus::Completed);
    }

    #[test]
    fn test_should_sync_never_synced() {
        let scheduler = SyncScheduler::new(SyncConfigManager::new());
        assert!(scheduler.should_sync("vault"));
    }

    #[test]
    fn test_scheduler_start_stop() {
        let scheduler = SyncScheduler::new(SyncConfigManager::new());
        scheduler.start();
        assert!(scheduler.is_running());
        scheduler.stop();
        assert!(!scheduler.is_running());
    }
}
