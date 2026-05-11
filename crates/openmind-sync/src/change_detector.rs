//! 内容哈希变更检测
//!
//! 利用SHA-256哈希检测内容是否变化。
//! 支持条目级别和批量检测。

use openmind_core::compute_content_hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 变更类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

/// 变更检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    /// 条目ID
    pub entry_id: String,
    /// 数据源ID
    pub source_id: String,
    /// 变更类型
    pub change_type: ChangeType,
    /// 旧哈希
    pub old_hash: Option<String>,
    /// 新哈希
    pub new_hash: Option<String>,
}

/// 内容哈希变更检测器
pub struct ChangeDetector {
    /// 已知的哈希映射 (source_id -> content_hash)
    known_hashes: HashMap<String, String>,
}

impl ChangeDetector {
    pub fn new() -> Self {
        Self {
            known_hashes: HashMap::new(),
        }
    }

    /// 从已有哈希映射创建
    pub fn with_hashes(hashes: HashMap<String, String>) -> Self {
        Self {
            known_hashes: hashes,
        }
    }

    /// 检测单个内容的变更
    pub fn detect(&self, source_id: &str, content: &str) -> ChangeRecord {
        let new_hash = compute_content_hash(content);

        match self.known_hashes.get(source_id) {
            None => ChangeRecord {
                entry_id: String::new(),
                source_id: source_id.to_string(),
                change_type: ChangeType::Added,
                old_hash: None,
                new_hash: Some(new_hash),
            },
            Some(old_hash) if old_hash == &new_hash => ChangeRecord {
                entry_id: String::new(),
                source_id: source_id.to_string(),
                change_type: ChangeType::Unchanged,
                old_hash: Some(old_hash.clone()),
                new_hash: Some(new_hash),
            },
            Some(old_hash) => ChangeRecord {
                entry_id: String::new(),
                source_id: source_id.to_string(),
                change_type: ChangeType::Modified,
                old_hash: Some(old_hash.clone()),
                new_hash: Some(new_hash),
            },
        }
    }

    /// 批量检测变更
    pub fn detect_batch(&self, items: &[(&str, &str)]) -> Vec<ChangeRecord> {
        items
            .iter()
            .map(|(source_id, content)| self.detect(source_id, content))
            .collect()
    }

    /// 检测删除（已知但不在当前列表中的条目）
    pub fn detect_deletions(&self, current_source_ids: &[&str]) -> Vec<ChangeRecord> {
        let current_set: std::collections::HashSet<&str> =
            current_source_ids.iter().copied().collect();

        self.known_hashes
            .keys()
            .filter(|id| !current_set.contains(id.as_str()))
            .map(|id| ChangeRecord {
                entry_id: String::new(),
                source_id: id.clone(),
                change_type: ChangeType::Deleted,
                old_hash: self.known_hashes.get(id).cloned(),
                new_hash: None,
            })
            .collect()
    }

    /// 更新已知哈希
    pub fn update_hash(&mut self, source_id: String, content_hash: String) {
        self.known_hashes.insert(source_id, content_hash);
    }

    /// 移除已知哈希（删除后清理）
    pub fn remove_hash(&mut self, source_id: &str) {
        self.known_hashes.remove(source_id);
    }

    /// 获取所有已知哈希
    pub fn known_hashes(&self) -> &HashMap<String, String> {
        &self.known_hashes
    }

    /// 仅过滤有变化的记录
    pub fn filter_changed(records: &[ChangeRecord]) -> Vec<&ChangeRecord> {
        records
            .iter()
            .filter(|r| r.change_type != ChangeType::Unchanged)
            .collect()
    }
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_added() {
        let detector = ChangeDetector::new();
        let record = detector.detect("doc1.md", "Hello world");
        assert_eq!(record.change_type, ChangeType::Added);
        assert!(record.new_hash.is_some());
    }

    #[test]
    fn test_detect_unchanged() {
        let mut hashes = HashMap::new();
        let hash = compute_content_hash("Hello world");
        hashes.insert("doc1.md".to_string(), hash);

        let detector = ChangeDetector::with_hashes(hashes);
        let record = detector.detect("doc1.md", "Hello world");
        assert_eq!(record.change_type, ChangeType::Unchanged);
    }

    #[test]
    fn test_detect_modified() {
        let mut hashes = HashMap::new();
        let hash = compute_content_hash("Hello world");
        hashes.insert("doc1.md".to_string(), hash);

        let detector = ChangeDetector::with_hashes(hashes);
        let record = detector.detect("doc1.md", "Hello updated world");
        assert_eq!(record.change_type, ChangeType::Modified);
        assert!(record.old_hash.is_some());
        assert!(record.new_hash.is_some());
        assert_ne!(record.old_hash, record.new_hash);
    }

    #[test]
    fn test_detect_deletions() {
        let mut hashes = HashMap::new();
        hashes.insert("doc1.md".to_string(), "hash1".to_string());
        hashes.insert("doc2.md".to_string(), "hash2".to_string());
        hashes.insert("doc3.md".to_string(), "hash3".to_string());

        let detector = ChangeDetector::with_hashes(hashes);
        let deletions = detector.detect_deletions(&["doc1.md", "doc3.md"]);
        assert_eq!(deletions.len(), 1);
        assert_eq!(deletions[0].source_id, "doc2.md");
        assert_eq!(deletions[0].change_type, ChangeType::Deleted);
    }

    #[test]
    fn test_batch_detect() {
        let mut hashes = HashMap::new();
        hashes.insert("doc1.md".to_string(), compute_content_hash("Old content"));

        let detector = ChangeDetector::with_hashes(hashes);
        let records = detector.detect_batch(&[
            ("doc1.md", "New content"),
            ("doc2.md", "Brand new"),
        ]);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].change_type, ChangeType::Modified);
        assert_eq!(records[1].change_type, ChangeType::Added);
    }

    #[test]
    fn test_filter_changed() {
        let records = vec![
            ChangeRecord {
                entry_id: String::new(),
                source_id: "a".to_string(),
                change_type: ChangeType::Added,
                old_hash: None,
                new_hash: Some("h1".to_string()),
            },
            ChangeRecord {
                entry_id: String::new(),
                source_id: "b".to_string(),
                change_type: ChangeType::Unchanged,
                old_hash: Some("h2".to_string()),
                new_hash: Some("h2".to_string()),
            },
            ChangeRecord {
                entry_id: String::new(),
                source_id: "c".to_string(),
                change_type: ChangeType::Modified,
                old_hash: Some("h3".to_string()),
                new_hash: Some("h4".to_string()),
            },
        ];

        let changed = ChangeDetector::filter_changed(&records);
        assert_eq!(changed.len(), 2);
    }
}
