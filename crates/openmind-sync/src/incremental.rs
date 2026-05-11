//! 增量同步机制
//!
//! 只同步变化部分，基于变更检测结果制定同步计划。

use crate::change_detector::{ChangeDetector, ChangeType};
use crate::conflict::{ConflictResolver, ConflictStrategy};

use openmind_core::{compute_content_hash, ContentChange, KnowledgeEntry, SyncState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 同步计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPlan {
    /// 连接器名称
    pub connector_name: String,
    /// 待新增条目
    pub to_add: Vec<String>,
    /// 待更新条目
    pub to_update: Vec<String>,
    /// 待删除条目
    pub to_delete: Vec<String>,
    /// 冲突条目
    pub conflicts: Vec<String>,
    /// 计划生成时间
    pub created_at: String,
}

impl SyncPlan {
    pub fn new(connector_name: &str) -> Self {
        Self {
            connector_name: connector_name.to_string(),
            to_add: Vec::new(),
            to_update: Vec::new(),
            to_delete: Vec::new(),
            conflicts: Vec::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// 是否需要同步
    pub fn has_changes(&self) -> bool {
        !self.to_add.is_empty() || !self.to_update.is_empty() || !self.to_delete.is_empty()
    }

    /// 变更总数
    pub fn total_changes(&self) -> usize {
        self.to_add.len() + self.to_update.len() + self.to_delete.len()
    }
}

/// 同步执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// 连接器名称
    pub connector_name: String,
    /// 成功新增数
    pub added: usize,
    /// 成功更新数
    pub updated: usize,
    /// 成功删除数
    pub deleted: usize,
    /// 冲突数
    pub conflicted: usize,
    /// 错误数
    pub errors: usize,
    /// 错误详情
    pub error_details: Vec<String>,
}

impl SyncResult {
    pub fn new(connector_name: &str) -> Self {
        Self {
            connector_name: connector_name.to_string(),
            added: 0,
            updated: 0,
            deleted: 0,
            conflicted: 0,
            errors: 0,
            error_details: Vec::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.errors == 0
    }
}

/// 增量同步器
pub struct IncrementalSync {
    /// 变更检测器
    change_detector: HashMap<String, ChangeDetector>,
    /// 冲突解决器
    #[allow(dead_code)]
    conflict_resolver: ConflictResolver,
    /// 同步状态持久化 (connector_name -> SyncState)
    sync_states: HashMap<String, SyncState>,
}

impl IncrementalSync {
    pub fn new(strategy: ConflictStrategy) -> Self {
        Self {
            change_detector: HashMap::new(),
            conflict_resolver: ConflictResolver::new(strategy),
            sync_states: HashMap::new(),
        }
    }

    /// 注册Connector的变更检测器
    pub fn register_detector(&mut self, connector_name: &str, existing_hashes: HashMap<String, String>) {
        self.change_detector.insert(
            connector_name.to_string(),
            ChangeDetector::with_hashes(existing_hashes),
        );
    }

    /// 制定同步计划
    pub fn plan(&self, connector_name: &str, changes: &[ContentChange]) -> SyncPlan {
        let mut plan = SyncPlan::new(connector_name);

        for change in changes {
            match change {
                ContentChange::Added(id) => {
                    plan.to_add.push(id.clone());
                }
                ContentChange::Modified(id) => {
                    // Check for potential conflicts
                    if self.has_conflict(connector_name, id) {
                        plan.conflicts.push(id.clone());
                    }
                    plan.to_update.push(id.clone());
                }
                ContentChange::Deleted(id) => {
                    plan.to_delete.push(id.clone());
                }
            }
        }

        plan
    }

    /// 使用变更检测器制定计划（基于内容对比）
    pub fn plan_from_content(
        &self,
        connector_name: &str,
        current_items: &[(&str, &str)], // (source_id, content)
        current_source_ids: &[&str],
    ) -> SyncPlan {
        let mut plan = SyncPlan::new(connector_name);

        if let Some(detector) = self.change_detector.get(connector_name) {
            // Detect changes
            let records = detector.detect_batch(current_items);
            for record in &records {
                match record.change_type {
                    ChangeType::Added => plan.to_add.push(record.source_id.clone()),
                    ChangeType::Modified => plan.to_update.push(record.source_id.clone()),
                    ChangeType::Unchanged => {}
                    ChangeType::Deleted => plan.to_delete.push(record.source_id.clone()),
                }
            }

            // Detect deletions
            let deletions = detector.detect_deletions(current_source_ids);
            for del in &deletions {
                plan.to_delete.push(del.source_id.clone());
            }
        } else {
            // No detector, treat all as new
            for (source_id, _) in current_items {
                plan.to_add.push(source_id.to_string());
            }
        }

        plan
    }

    /// 执行同步
    pub fn execute(
        &mut self,
        connector_name: &str,
        plan: &SyncPlan,
        fetch_fn: impl Fn(&str) -> Option<KnowledgeEntry>,
        delete_fn: impl Fn(&str),
    ) -> SyncResult {
        let mut result = SyncResult::new(connector_name);

        // Process additions
        for id in &plan.to_add {
            if let Some(_entry) = fetch_fn(id) {
                result.added += 1;
            } else {
                result.errors += 1;
                result.error_details.push(format!("Failed to fetch added item: {}", id));
            }
        }

        // Process updates
        for id in &plan.to_update {
            if plan.conflicts.contains(id) {
                result.conflicted += 1;
            } else if let Some(_entry) = fetch_fn(id) {
                result.updated += 1;
            } else {
                result.errors += 1;
                result.error_details.push(format!("Failed to fetch updated item: {}", id));
            }
        }

        // Process deletions
        for id in &plan.to_delete {
            delete_fn(id);
            result.deleted += 1;
        }

        // Update sync state
        let now = chrono::Utc::now();
        let sync_state = SyncState {
            connector_name: connector_name.to_string(),
            last_sync_at: now,
            content_hash: None,
            status: if result.errors == 0 { "success".to_string() } else { "partial".to_string() },
            last_error: result.error_details.first().cloned(),
            total_synced: (result.added + result.updated + result.deleted) as i64,
            total_errors: result.errors as i64,
        };
        self.sync_states.insert(connector_name.to_string(), sync_state);

        result
    }

    /// 级联删除处理
    pub fn cascade_delete(
        entry_id: &str,
        get_related_fn: impl Fn(&str) -> Vec<String>,
        delete_fn: impl Fn(&str),
    ) -> Vec<String> {
        let mut deleted = Vec::new();
        let mut to_process = vec![entry_id.to_string()];
        let mut visited = std::collections::HashSet::new();

        while let Some(id) = to_process.pop() {
            if visited.contains(&id) {
                continue;
            }
            visited.insert(id.clone());

            // Get related entries
            let related = get_related_fn(&id);
            delete_fn(&id);
            deleted.push(id.clone());

            // Add related entries for cascade processing
            for related_id in related {
                if !visited.contains(&related_id) {
                    to_process.push(related_id);
                }
            }
        }

        deleted
    }

    /// 获取同步状态
    pub fn get_sync_state(&self, connector_name: &str) -> Option<&SyncState> {
        self.sync_states.get(connector_name)
    }

    /// 保存同步状态（持久化）
    pub fn persist_sync_states(&self) -> HashMap<String, SyncState> {
        self.sync_states.clone()
    }

    /// 加载同步状态
    pub fn load_sync_states(&mut self, states: HashMap<String, SyncState>) {
        self.sync_states.extend(states);
    }

    /// 检查是否有冲突
    fn has_conflict(&self, _connector_name: &str, _entry_id: &str) -> bool {
        // Simplified: in real implementation, would check if local entry
        // was modified after last sync
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_plan() {
        let plan = SyncPlan::new("vault");
        assert!(!plan.has_changes());
        assert_eq!(plan.total_changes(), 0);
    }

    #[test]
    fn test_sync_plan_with_changes() {
        let mut plan = SyncPlan::new("vault");
        plan.to_add.push("doc1".to_string());
        plan.to_update.push("doc2".to_string());
        plan.to_delete.push("doc3".to_string());
        assert!(plan.has_changes());
        assert_eq!(plan.total_changes(), 3);
    }

    #[test]
    fn test_incremental_sync_plan() {
        let mut sync = IncrementalSync::new(ConflictStrategy::LastWriteWins);
        sync.register_detector("vault", HashMap::new());

        let changes = vec![
            ContentChange::Added("doc1".to_string()),
            ContentChange::Modified("doc2".to_string()),
            ContentChange::Deleted("doc3".to_string()),
        ];

        let plan = sync.plan("vault", &changes);
        assert_eq!(plan.to_add.len(), 1);
        assert_eq!(plan.to_update.len(), 1);
        assert_eq!(plan.to_delete.len(), 1);
    }

    #[test]
    fn test_incremental_sync_execute() {
        let mut sync = IncrementalSync::new(ConflictStrategy::LastWriteWins);

        let plan = SyncPlan::new("vault");
        let result = sync.execute(
            "vault",
            &plan,
            |_id| None,
            |_id| {},
        );
        assert!(result.is_success());
    }

    #[test]
    fn test_cascade_delete() {
        let deleted = IncrementalSync::cascade_delete(
            "entry1",
            |id| {
                match id {
                    "entry1" => vec!["entry2".to_string()],
                    "entry2" => vec!["entry3".to_string()],
                    _ => vec![],
                }
            },
            |_id| {},
        );
        assert!(deleted.contains(&"entry1".to_string()));
        assert!(deleted.contains(&"entry2".to_string()));
        assert!(deleted.contains(&"entry3".to_string()));
    }

    #[test]
    fn test_sync_state_persistence() {
        let mut sync = IncrementalSync::new(ConflictStrategy::LastWriteWins);
        let plan = SyncPlan::new("vault");
        sync.execute("vault", &plan, |_id| None, |_id| {});

        let states = sync.persist_sync_states();
        assert!(states.contains_key("vault"));

        // Load into new instance
        let mut sync2 = IncrementalSync::new(ConflictStrategy::LastWriteWins);
        sync2.load_sync_states(states);
        assert!(sync2.get_sync_state("vault").is_some());
    }

    #[test]
    fn test_plan_from_content() {
        let mut sync = IncrementalSync::new(ConflictStrategy::LastWriteWins);
        let mut hashes = HashMap::new();
        hashes.insert("doc1.md".to_string(), compute_content_hash("Old content"));
        sync.register_detector("vault", hashes);

        let plan = sync.plan_from_content(
            "vault",
            &[("doc1.md", "New content"), ("doc2.md", "Brand new")],
            &["doc1.md", "doc2.md"],
        );

        assert!(plan.to_update.contains(&"doc1.md".to_string()));
        assert!(plan.to_add.contains(&"doc2.md".to_string()));
    }
}
