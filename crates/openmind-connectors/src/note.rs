//! 备忘录同步Connector
//!
//! 从纯文本/Markdown备忘录目录同步笔记到知识库。

use async_trait::async_trait;
use openmind_core::connector_registry::{ConnectorCapabilities, EnhancedConnector};
use openmind_core::{compute_content_hash, Connector, ContentChange, ContentItem};
use serde::{Deserialize, Serialize};

/// 备忘录Connector配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteConfig {
    /// 备忘录目录路径
    pub notes_path: String,
    /// 默认标签
    pub default_tags: Vec<String>,
    /// 文件扩展名
    pub extensions: Vec<String>,
}

impl Default for NoteConfig {
    fn default() -> Self {
        Self {
            notes_path: String::new(),
            default_tags: vec!["note".to_string()],
            extensions: vec!["md".to_string(), "txt".to_string()],
        }
    }
}

/// 备忘录Connector
///
/// 从本地目录同步备忘录。每条笔记视为一个知识条目。
/// 支持增量同步（基于文件修改时间）。
pub struct NoteConnector {
    /// 配置
    config: NoteConfig,
}

impl NoteConnector {
    /// 创建新的NoteConnector
    pub fn new(config: NoteConfig) -> Self {
        Self { config }
    }

    /// 使用路径创建
    pub fn with_path(notes_path: impl Into<String>) -> Self {
        let mut config = NoteConfig::default();
        config.notes_path = notes_path.into();
        Self { config }
    }

    /// 从笔记文件名提取标题
    fn extract_title_from_filename(&self, path: &std::path::Path) -> String {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .replace('-', " ")
            .replace('_', " ")
    }

    /// 从内容提取标签（#tag格式）
    fn extract_tags_from_content(&self, content: &str) -> Vec<String> {
        let mut tags = Vec::new();
        for line in content.lines() {
            for word in line.split_whitespace() {
                if word.starts_with('#') && word.len() > 1 {
                    let tag = word.trim_start_matches('#').to_string();
                    if !tag.is_empty() && !tags.contains(&tag) {
                        tags.push(tag);
                    }
                }
            }
        }
        tags
    }

    /// 扫描笔记文件
    fn scan_notes(&self) -> anyhow::Result<Vec<std::path::PathBuf>> {
        let root = std::path::Path::new(&self.config.notes_path);
        if !root.exists() {
            anyhow::bail!("Notes path does not exist: {}", self.config.notes_path);
        }

        let mut files = Vec::new();
        let entries = std::fs::read_dir(root)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if self.config.extensions.contains(&ext.to_string()) {
                    files.push(path);
                }
            }
        }
        Ok(files)
    }
}

#[async_trait]
impl Connector for NoteConnector {
    fn name(&self) -> &str {
        "note"
    }

    async fn connect(&self) -> anyhow::Result<()> {
        let root = std::path::Path::new(&self.config.notes_path);
        if !root.exists() {
            anyhow::bail!("Notes path does not exist: {}", self.config.notes_path);
        }
        tracing::info!("Note connector connected to: {}", self.config.notes_path);
        Ok(())
    }

    async fn list_changes(
        &self,
        since: &openmind_core::SyncState,
    ) -> anyhow::Result<Vec<ContentChange>> {
        let files = self.scan_notes()?;
        let mut changes = Vec::new();

        for file in files {
            let file_id = file.to_string_lossy().to_string();

            if let Ok(metadata) = std::fs::metadata(&file) {
                if let Ok(modified) = metadata.modified() {
                    let modified_time: chrono::DateTime<chrono::Utc> = modified.into();
                    if modified_time > since.last_sync_at {
                        changes.push(ContentChange::Modified(file_id));
                    }
                }
            } else {
                changes.push(ContentChange::Added(file_id));
            }
        }

        Ok(changes)
    }

    async fn fetch_content(&self, id: &str) -> anyhow::Result<ContentItem> {
        let path = std::path::Path::new(id);
        if !path.exists() {
            anyhow::bail!("Note not found: {}", id);
        }

        let content = std::fs::read_to_string(path)?;
        let content_hash = compute_content_hash(&content);
        let title = self.extract_title_from_filename(path);

        // Extract tags from content
        let mut tags = self.config.default_tags.clone();
        for tag in self.extract_tags_from_content(&content) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }

        // Determine content type
        let content_type = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| match ext {
                "md" | "markdown" => "markdown",
                _ => "text",
            })
            .unwrap_or("text")
            .to_string();

        Ok(ContentItem {
            source: id.to_string(),
            content_type,
            content,
            title: Some(title),
            metadata: serde_json::json!({
                "content_hash": content_hash,
                "connector": "note",
            }),
            file_references: vec![],
            tags,
        })
    }
}

#[async_trait]
impl EnhancedConnector for NoteConnector {
    fn capabilities(&self) -> ConnectorCapabilities {
        ConnectorCapabilities::new(vec!["markdown", "text"], "poll", "incremental")
            .with_capability(openmind_core::Capability::new(
                "hashtag_extraction",
                "Extract #tags from note content",
            ))
            .with_capability(openmind_core::Capability::new(
                "incremental_sync",
                "Only sync notes modified since last sync",
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title_from_filename() {
        let connector = NoteConnector::with_path("/tmp/test");

        let title = connector.extract_title_from_filename(std::path::Path::new("my-test-note.md"));
        assert_eq!(title, "my test note");

        let title = connector.extract_title_from_filename(std::path::Path::new("simple.txt"));
        assert_eq!(title, "simple");
    }

    #[test]
    fn test_extract_tags_from_content() {
        let connector = NoteConnector::with_path("/tmp/test");

        let content = "This is a note #rust #programming\nMore text #test";
        let tags = connector.extract_tags_from_content(content);

        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"programming".to_string()));
        assert!(tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_extract_tags_no_duplicates() {
        let connector = NoteConnector::with_path("/tmp/test");
        let content = "#rust and #rust again";
        let tags = connector.extract_tags_from_content(content);
        assert_eq!(tags.len(), 1);
    }

    #[tokio::test]
    async fn test_note_fetch_content() {
        let dir = tempfile::tempdir().unwrap();
        let note_path = dir.path().join("test-note.md");
        std::fs::write(&note_path, "# My Note\n\nSome content #rust #test").unwrap();

        let connector = NoteConnector::with_path(dir.path().to_string_lossy().to_string());
        let item = connector
            .fetch_content(&note_path.to_string_lossy())
            .await
            .unwrap();

        assert_eq!(item.title, Some("test note".to_string()));
        assert!(item.tags.contains(&"note".to_string())); // default
        assert!(item.tags.contains(&"rust".to_string()));
    }

    #[tokio::test]
    async fn test_note_capabilities() {
        let connector = NoteConnector::with_path("/tmp/test");
        let caps = connector.capabilities();
        assert!(caps.supported_formats.contains(&"markdown".to_string()));
        assert_eq!(caps.sync_mode, "incremental");
    }
}
