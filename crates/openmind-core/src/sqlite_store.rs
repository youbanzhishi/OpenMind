//! SQLite元数据存储实现
//!
//! 基于rusqlite的知识存储，包含：
//! - knowledge_entries 表
//! - file_references 表
//! - knowledge_relations 表
//! - sync_states 表
//! - knowledge_fts FTS5全文索引

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};


use crate::models::*;
use crate::traits::KnowledgeStore;

/// SQLite知识存储
pub struct SqliteKnowledgeStore {
    conn: Mutex<Connection>,
}

impl SqliteKnowledgeStore {
    /// 打开或创建SQLite数据库
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// 在内存中创建数据库（用于测试）
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// 初始化数据库Schema
    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS knowledge_entries (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                embedding_id TEXT,
                embedding_status TEXT NOT NULL DEFAULT 'pending',
                tags TEXT NOT NULL DEFAULT '[]',
                project TEXT,
                metadata TEXT NOT NULL DEFAULT '{}',
                file_references TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_entries_source ON knowledge_entries(source_type, source_id);
            CREATE INDEX IF NOT EXISTS idx_entries_hash ON knowledge_entries(content_hash);
            CREATE INDEX IF NOT EXISTS idx_entries_status ON knowledge_entries(status);
            CREATE INDEX IF NOT EXISTS idx_entries_updated ON knowledge_entries(updated_at);
            CREATE INDEX IF NOT EXISTS idx_entries_embedding_status ON knowledge_entries(embedding_status);

            CREATE TABLE IF NOT EXISTS knowledge_relations (
                id TEXT PRIMARY KEY,
                from_id TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
                to_id TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
                relation_type TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                UNIQUE(from_id, to_id, relation_type)
            );

            CREATE INDEX IF NOT EXISTS idx_relations_from ON knowledge_relations(from_id);
            CREATE INDEX IF NOT EXISTS idx_relations_to ON knowledge_relations(to_id);
            CREATE INDEX IF NOT EXISTS idx_relations_type ON knowledge_relations(relation_type);

            CREATE TABLE IF NOT EXISTS sync_states (
                connector_name TEXT PRIMARY KEY,
                last_sync_at TEXT NOT NULL,
                content_hash TEXT,
                status TEXT NOT NULL DEFAULT 'idle',
                last_error TEXT,
                total_synced INTEGER NOT NULL DEFAULT 0,
                total_errors INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
                id,
                title,
                content,
                tags,
                tokenize='unicode61'
            );

            -- FTS sync is done manually in store/delete methods
            ",
        )?;
        Ok(())
    }

    /// 从行数据构建KnowledgeEntry
    fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<KnowledgeEntry> {
        let embedding_status_str: String = row.get(7)?;
        let source_type_str: String = row.get(1)?;
        let status_str: String = row.get(12)?;
        let tags_str: String = row.get(8)?;
        let metadata_str: String = row.get(10)?;
        let file_refs_str: String = row.get(11)?;
        let created_at_str: String = row.get(13)?;
        let updated_at_str: String = row.get(14)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(KnowledgeEntry {
            id: row.get(0)?,
            source_type: SourceType::from_str(&source_type_str).unwrap_or(SourceType::File),
            source_id: row.get(2)?,
            title: row.get(3)?,
            content: row.get(4)?,
            content_hash: row.get(5)?,
            embedding_id: row.get(6)?,
            embedding_status: EmbeddingStatus::from_str_simple(&embedding_status_str),
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            project: row.get(9)?,
            metadata: serde_json::from_str(&metadata_str).unwrap_or(serde_json::Value::Null),
            file_references: serde_json::from_str(&file_refs_str).unwrap_or_default(),
            created_at,
            updated_at,
            status: EntryStatus::from_str(&status_str).unwrap_or(EntryStatus::Active),
        })
    }
}

/// 计算内容哈希
pub fn compute_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[async_trait]
impl KnowledgeStore for SqliteKnowledgeStore {
    async fn store(&self, entry: KnowledgeEntry) -> anyhow::Result<String> {
        let conn = self.conn.lock().unwrap();
        let tags_json = serde_json::to_string(&entry.tags)?;
        let metadata_json = serde_json::to_string(&entry.metadata)?;
        let file_refs_json = serde_json::to_string(&entry.file_references)?;
        let embedding_status_str = entry.embedding_status.as_str();

        // Delete from FTS first (for INSERT OR REPLACE case)
        conn.execute(
            "DELETE FROM knowledge_fts WHERE id = ?1",
            params![entry.id],
        ).ok(); // Ignore error if not exists

        conn.execute(
            "INSERT OR REPLACE INTO knowledge_entries
             (id, source_type, source_id, title, content, content_hash, embedding_id, embedding_status,
              tags, project, metadata, file_references, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                entry.id,
                entry.source_type.as_str(),
                entry.source_id,
                entry.title,
                entry.content,
                entry.content_hash,
                entry.embedding_id,
                embedding_status_str,
                tags_json.clone(),
                entry.project,
                metadata_json,
                file_refs_json,
                entry.status.as_str(),
                entry.created_at.to_rfc3339(),
                entry.updated_at.to_rfc3339(),
            ],
        )?;

        // Insert into FTS
        conn.execute(
            "INSERT INTO knowledge_fts(id, title, content, tags) VALUES (?1, ?2, ?3, ?4)",
            params![entry.id, entry.title, entry.content, tags_json],
        )?;

        Ok(entry.id)
    }

    async fn query_semantic(
        &self,
        _query: &str,
        _embedding: &[f32],
        _limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        // Semantic search requires Qdrant - return empty with degraded notice
        Ok(vec![])
    }

    async fn query_keyword(
        &self,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let conn = self.conn.lock().unwrap();

        // Simple approach: FTS search first, then filter in memory if needed
        // This avoids dynamic SQL parameter binding issues
        let sql = "SELECT e.id, e.source_type, e.source_id, e.title, e.content,
                    e.content_hash, e.embedding_id, e.embedding_status,
                    e.tags, e.project, e.metadata, e.file_references,
                    e.status, e.created_at, e.updated_at,
                    fts.rank
             FROM knowledge_fts fts
             JOIN knowledge_entries e ON fts.id = e.id
             WHERE knowledge_fts MATCH ?1
             ORDER BY fts.rank";

        let mut stmt = conn.prepare(sql)?;
        let fts_results: Vec<(KnowledgeEntry, f64)> = stmt
            .query_map(params![query], |row| {
                let entry = Self::row_to_entry(row)?;
                let rank: f64 = row.get(15)?;
                Ok((entry, rank))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Apply filters in memory
        let mut results: Vec<SearchResult> = fts_results
            .into_iter()
            .filter(|(entry, _)| {
                if let Some(ref source) = filters.source {
                    if entry.source_type.as_str() != source.as_str() {
                        return false;
                    }
                }
                if let Some(ref project) = filters.project {
                    if entry.project.as_deref() != Some(project.as_str()) {
                        return false;
                    }
                }
                if !filters.tags.is_empty() {
                    let all_match = filters.tags.iter().all(|t| entry.tags.contains(t));
                    if !all_match {
                        return false;
                    }
                }
                if let Some(ref date_from) = filters.date_from {
                    if entry.created_at < *date_from {
                        return false;
                    }
                }
                if let Some(ref date_to) = filters.date_to {
                    if entry.created_at > *date_to {
                        return false;
                    }
                }
                true
            })
            .map(|(entry, rank)| SearchResult {
                entry,
                relevance: 1.0 / (1.0 + (-rank).exp()),
                highlights: vec![],
            })
            .collect();

        results.truncate(limit);
        Ok(results)
    }


    async fn relate(&self, relation: KnowledgeRelation) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let metadata_json = serde_json::to_string(&relation.metadata)?;
        conn.execute(
            "INSERT OR REPLACE INTO knowledge_relations
             (id, from_id, to_id, relation_type, weight, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                relation.id,
                relation.from_id,
                relation.to_id,
                relation.relation_type,
                relation.weight,
                metadata_json,
                relation.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    async fn get_related(
        &self,
        entry_id: &str,
        depth: usize,
    ) -> anyhow::Result<Vec<KnowledgeRelation>> {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();
        let mut current_ids = vec![entry_id.to_string()];
        let mut visited = std::collections::HashSet::new();
        visited.insert(entry_id.to_string());

        for _ in 0..depth {
            let mut next_ids = Vec::new();
            for id in &current_ids {
                let mut stmt = conn.prepare(
                    "SELECT id, from_id, to_id, relation_type, weight, metadata, created_at
                     FROM knowledge_relations WHERE from_id = ?1 OR to_id = ?1"
                )?;
                let rows = stmt.query_map(params![id], |row| {
                    let metadata_str: String = row.get(5)?;
                    let created_at_str: String = row.get(6)?;
                    Ok(KnowledgeRelation {
                        id: row.get(0)?,
                        from_id: row.get(1)?,
                        to_id: row.get(2)?,
                        relation_type: row.get(3)?,
                        weight: row.get(4)?,
                        metadata: serde_json::from_str(&metadata_str).unwrap_or(serde_json::Value::Null),
                        created_at: DateTime::parse_from_rfc3339(&created_at_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                    })
                })?;

                for row in rows {
                    if let Ok(relation) = row {
                        let other_id = if relation.from_id == *id {
                            &relation.to_id
                        } else {
                            &relation.from_id
                        };
                        if !visited.contains(other_id) {
                            visited.insert(other_id.clone());
                            next_ids.push(other_id.clone());
                        }
                        results.push(relation);
                    }
                }
            }
            current_ids = next_ids;
        }

        Ok(results)
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<KnowledgeEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, title, content, content_hash,
                    embedding_id, embedding_status, tags, project, metadata,
                    file_references, status, created_at, updated_at
             FROM knowledge_entries WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_entry(row)?))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        // Delete from FTS
        conn.execute("DELETE FROM knowledge_fts WHERE id = ?1", params![id]).ok();
        // Delete from main table
        conn.execute("DELETE FROM knowledge_entries WHERE id = ?1", params![id])?;
        Ok(())
    }

    async fn stats(&self) -> anyhow::Result<KnowledgeStats> {
        let conn = self.conn.lock().unwrap();
        let total_entries: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_entries",
            [],
            |row| row.get(0),
        )?;

        let total_relations: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_relations",
            [],
            |row| row.get(0),
        )?;

        // Count by source_type
        let mut stmt = conn.prepare(
            "SELECT source_type, COUNT(*) FROM knowledge_entries GROUP BY source_type"
        )?;
        let mut by_source = serde_json::Map::new();
        let rows = stmt.query_map([], |row| {
            let st: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((st, cnt))
        })?;
        for row in rows {
            if let Ok((st, cnt)) = row {
                by_source.insert(st, serde_json::Value::Number(cnt.into()));
            }
        }

        // Count by embedding_status
        let mut stmt = conn.prepare(
            "SELECT embedding_status, COUNT(*) FROM knowledge_entries GROUP BY embedding_status"
        )?;
        let mut by_embedding = serde_json::Map::new();
        let rows = stmt.query_map([], |row| {
            let st: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((st, cnt))
        })?;
        for row in rows {
            if let Ok((st, cnt)) = row {
                by_embedding.insert(st, serde_json::Value::Number(cnt.into()));
            }
        }

        // Count unique tags
        let total_tags: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT json_each.value) FROM knowledge_entries, json_each(tags)",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(KnowledgeStats {
            total_entries,
            by_source: serde_json::Value::Object(by_source),
            by_embedding_status: serde_json::Value::Object(by_embedding),
            total_relations,
            total_tags,
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_store_basic_crud() {
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let now = Utc::now();

        let entry = KnowledgeEntry {
            id: uuid::Uuid::new_v4().to_string(),
            source_type: SourceType::File,
            source_id: "test.md".to_string(),
            title: "Test Entry".to_string(),
            content: "This is a test entry about Rust programming.".to_string(),
            content_hash: compute_content_hash("This is a test entry about Rust programming."),
            embedding_id: None,
            embedding_status: EmbeddingStatus::Pending,
            tags: vec!["rust".to_string(), "test".to_string()],
            project: Some("test-project".to_string()),
            metadata: serde_json::json!({"key": "value"}),
            file_references: vec![],
            created_at: now,
            updated_at: now,
            status: EntryStatus::Active,
        };

        let entry_id = entry.id.clone();

        // Store
        let id = store.store(entry).await.unwrap();
        assert_eq!(id, entry_id);

        // Get
        let got = store.get(&entry_id).await.unwrap().unwrap();
        assert_eq!(got.title, "Test Entry");
        assert_eq!(got.source_type, SourceType::File);

        // Delete
        store.delete(&entry_id).await.unwrap();
        let got = store.get(&entry_id).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_store_relations() {
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let now = Utc::now();

        let entry1 = KnowledgeEntry {
            id: uuid::Uuid::new_v4().to_string(),
            source_type: SourceType::File,
            source_id: "a.md".to_string(),
            title: "Entry A".to_string(),
            content: "Content A".to_string(),
            content_hash: compute_content_hash("Content A"),
            embedding_id: None,
            embedding_status: EmbeddingStatus::Pending,
            tags: vec![],
            project: None,
            metadata: serde_json::Value::Null,
            file_references: vec![],
            created_at: now,
            updated_at: now,
            status: EntryStatus::Active,
        };

        let entry2 = KnowledgeEntry {
            id: uuid::Uuid::new_v4().to_string(),
            source_type: SourceType::File,
            source_id: "b.md".to_string(),
            title: "Entry B".to_string(),
            content: "Content B".to_string(),
            content_hash: compute_content_hash("Content B"),
            embedding_id: None,
            embedding_status: EmbeddingStatus::Pending,
            tags: vec![],
            project: None,
            metadata: serde_json::Value::Null,
            file_references: vec![],
            created_at: now,
            updated_at: now,
            status: EntryStatus::Active,
        };

        let id1 = entry1.id.clone();
        let id2 = entry2.id.clone();

        store.store(entry1).await.unwrap();
        store.store(entry2).await.unwrap();

        let relation = KnowledgeRelation {
            id: uuid::Uuid::new_v4().to_string(),
            from_id: id1.clone(),
            to_id: id2.clone(),
            relation_type: "similar_to".to_string(),
            weight: 0.85,
            metadata: serde_json::Value::Null,
            created_at: now,
        };

        store.relate(relation).await.unwrap();

        let related = store.get_related(&id1, 1).await.unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].relation_type, "similar_to");
    }

    #[tokio::test]
    async fn test_sqlite_store_fts_search() {
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let now = Utc::now();

        // Insert entries
        let entries = vec![
            ("Rust Guide", "Rust is a systems programming language focused on safety."),
            ("Python Tips", "Python is great for data science and machine learning."),
            ("Rust vs Go", "Comparing Rust and Go for backend development."),
        ];

        for (i, (title, content)) in entries.iter().enumerate() {
            let entry = KnowledgeEntry {
                id: uuid::Uuid::new_v4().to_string(),
                source_type: SourceType::File,
                source_id: format!("doc{}.md", i),
                title: title.to_string(),
                content: content.to_string(),
                content_hash: compute_content_hash(content),
                embedding_id: None,
                embedding_status: EmbeddingStatus::Pending,
                tags: vec![],
                project: None,
                metadata: serde_json::Value::Null,
                file_references: vec![],
                created_at: now,
                updated_at: now,
                status: EntryStatus::Active,
            };
            store.store(entry).await.unwrap();
        }

        // Search for "Rust"
        let filters = SearchFilters::default();
        let results = store.query_keyword("Rust", 10, &filters).await.unwrap();
        assert!(results.len() >= 2, "Should find at least 2 results for 'Rust'");
    }

    #[test]
    fn test_content_hash() {
        let hash1 = compute_content_hash("hello world");
        let hash2 = compute_content_hash("hello world");
        let hash3 = compute_content_hash("goodbye world");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA-256 hex
    }

    #[tokio::test]
    async fn test_stats() {
        let store = SqliteKnowledgeStore::open_in_memory().unwrap();
        let stats = store.stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_relations, 0);
    }
}
