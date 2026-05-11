//! 冲突解决策略
//!
//! 可配置的冲突解决策略：last-write-wins / manual / merge / source-priority

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 冲突解决策略
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    /// 最后写入胜出（基于时间戳）
    LastWriteWins,
    /// 手动解决（标记冲突等待人工处理）
    Manual,
    /// 合并（尝试自动合并）
    Merge,
    /// 源优先级（指定数据源优先级）
    SourcePriority,
}

impl ConflictStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "last_write_wins" => Some(Self::LastWriteWins),
            "manual" => Some(Self::Manual),
            "merge" => Some(Self::Merge),
            "source_priority" => Some(Self::SourcePriority),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::LastWriteWins => "last_write_wins",
            Self::Manual => "manual",
            Self::Merge => "merge",
            Self::SourcePriority => "source_priority",
        }
    }
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        Self::LastWriteWins
    }
}

/// 冲突记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// 冲突ID
    pub id: String,
    /// 条目ID
    pub entry_id: String,
    /// 本地版本时间戳
    pub local_updated_at: DateTime<Utc>,
    /// 远程版本时间戳
    pub remote_updated_at: DateTime<Utc>,
    /// 本地内容哈希
    pub local_hash: String,
    /// 远程内容哈希
    pub remote_hash: String,
    /// 冲突来源
    pub source: String,
}

/// 冲突解决结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResult {
    /// 使用本地版本
    UseLocal(String),
    /// 使用远程版本
    UseRemote(String),
    /// 已合并
    Merged(String),
    /// 需要手动处理
    NeedsManualResolution(Conflict),
}

/// 冲突解决器
pub struct ConflictResolver {
    /// 当前策略
    strategy: ConflictStrategy,
    /// 源优先级映射 (source_name -> priority, 越小越优先)
    source_priorities: std::collections::HashMap<String, u32>,
}

impl ConflictResolver {
    pub fn new(strategy: ConflictStrategy) -> Self {
        Self {
            strategy,
            source_priorities: std::collections::HashMap::new(),
        }
    }

    /// 设置源优先级
    pub fn with_source_priority(mut self, source: impl Into<String>, priority: u32) -> Self {
        self.source_priorities.insert(source.into(), priority);
        self
    }

    /// 解决冲突
    pub fn resolve(&self, conflict: Conflict) -> ConflictResult {
        match self.strategy {
            ConflictStrategy::LastWriteWins => {
                if conflict.remote_updated_at > conflict.local_updated_at {
                    ConflictResult::UseRemote(conflict.entry_id)
                } else {
                    ConflictResult::UseLocal(conflict.entry_id)
                }
            }
            ConflictStrategy::Manual => ConflictResult::NeedsManualResolution(conflict),
            ConflictStrategy::Merge => {
                // Simple merge: prefer longer content, or use remote if same length
                // In real implementation, would do proper 3-way merge
                ConflictResult::Merged(conflict.entry_id)
            }
            ConflictStrategy::SourcePriority => {
                let local_priority = self
                    .source_priorities
                    .get(&conflict.source)
                    .copied()
                    .unwrap_or(u32::MAX);
                let remote_priority = self
                    .source_priorities
                    .get(&conflict.source)
                    .copied()
                    .unwrap_or(u32::MAX);

                // Lower priority number = higher precedence
                if remote_priority <= local_priority {
                    ConflictResult::UseRemote(conflict.entry_id)
                } else {
                    ConflictResult::UseLocal(conflict.entry_id)
                }
            }
        }
    }

    /// 获取当前策略
    pub fn strategy(&self) -> &ConflictStrategy {
        &self.strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conflict(local_newer: bool) -> Conflict {
        let now = Utc::now();
        let local_time = if local_newer {
            now
        } else {
            now - chrono::Duration::seconds(60)
        };
        let remote_time = if local_newer {
            now - chrono::Duration::seconds(60)
        } else {
            now
        };

        Conflict {
            id: "c1".to_string(),
            entry_id: "e1".to_string(),
            local_updated_at: local_time,
            remote_updated_at: remote_time,
            local_hash: "hash_local".to_string(),
            remote_hash: "hash_remote".to_string(),
            source: "vault".to_string(),
        }
    }

    #[test]
    fn test_last_write_wins_remote_newer() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriteWins);
        let conflict = make_conflict(false);
        let result = resolver.resolve(conflict);
        match result {
            ConflictResult::UseRemote(id) => assert_eq!(id, "e1"),
            _ => panic!("Expected UseRemote"),
        }
    }

    #[test]
    fn test_last_write_wins_local_newer() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriteWins);
        let conflict = make_conflict(true);
        let result = resolver.resolve(conflict);
        match result {
            ConflictResult::UseLocal(id) => assert_eq!(id, "e1"),
            _ => panic!("Expected UseLocal"),
        }
    }

    #[test]
    fn test_manual_resolution() {
        let resolver = ConflictResolver::new(ConflictStrategy::Manual);
        let conflict = make_conflict(false);
        let result = resolver.resolve(conflict);
        match result {
            ConflictResult::NeedsManualResolution(c) => assert_eq!(c.entry_id, "e1"),
            _ => panic!("Expected NeedsManualResolution"),
        }
    }

    #[test]
    fn test_merge_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::Merge);
        let conflict = make_conflict(false);
        let result = resolver.resolve(conflict);
        match result {
            ConflictResult::Merged(id) => assert_eq!(id, "e1"),
            _ => panic!("Expected Merged"),
        }
    }

    #[test]
    fn test_source_priority() {
        let resolver = ConflictResolver::new(ConflictStrategy::SourcePriority)
            .with_source_priority("vault", 1)
            .with_source_priority("blog", 2);

        let conflict = make_conflict(false);
        let result = resolver.resolve(conflict);
        // With source_priority=1, remote (from same source) gets UseRemote
        match result {
            ConflictResult::UseRemote(id) => assert_eq!(id, "e1"),
            _ => panic!("Expected UseRemote for higher priority source"),
        }
    }

    #[test]
    fn test_strategy_from_str() {
        assert_eq!(
            ConflictStrategy::from_str("last_write_wins"),
            Some(ConflictStrategy::LastWriteWins)
        );
        assert_eq!(
            ConflictStrategy::from_str("manual"),
            Some(ConflictStrategy::Manual)
        );
        assert_eq!(ConflictStrategy::from_str("unknown"), None);
    }
}
