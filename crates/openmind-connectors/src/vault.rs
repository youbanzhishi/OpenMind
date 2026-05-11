//! OpenVault文件同步Connector
//!
//! 同步本地文件系统中的Markdown/文本文件到知识库。
//! 支持增量同步（基于文件修改时间）和内容哈希变更检测。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use openmind_core::{
    Connector, ContentChange, ContentItem,
    compute_content_hash,
};
use openmind_core::connector_registry::{ConnectorCapabilities, EnhancedConnector};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// OpenVault Connector配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Vault根目录
    pub root_path: String,
    /// 要包含的文件扩展名
    pub extensions: Vec<String>,
    /// 是否递归子目录
    pub recursive: bool,
    /// 排除的目录名
    pub exclude_dirs: Vec<String>,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            root_path: String::new(),
            extensions: vec!["md".to_string(), "txt".to_string(), "markdown".to_string()],
            recursive: true,
            exclude_dirs: vec![".git".to_string(), ".obsidian".to_string(), "node_modules".to_string()],
        }
    }
}

/// OpenVault Connector
///
/// 将本地Vault目录中的文件同步到OpenMind知识库。
/// 支持增量同步：只同步修改时间晚于last_sync_at的文件。
pub struct VaultConnector {
    /// 配置
    config: VaultConfig,
}

impl VaultConnector {
    /// 创建新的VaultConnector
    pub fn new(config: VaultConfig) -> Self {
        Self { config }
    }

    /// 使用默认配置
    pub fn with_path(root_path: impl Into<String>) -> Self {
        let mut config = VaultConfig::default();
        config.root_path = root_path.into();
        Self { config }
    }

    /// 检查文件扩展名是否匹配
    fn is_matching_extension(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| self.config.extensions.contains(&ext.to_string()))
            .unwrap_or(false)
    }

    /// 检查路径是否在排除目录中
    fn is_excluded(&self, path: &Path) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                if let Some(name) = os_str.to_str() {
                    if self.config.exclude_dirs.contains(&name.to_string()) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 扫描目录中的文件
    fn scan_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        let root = Path::new(&self.config.root_path);
        if !root.exists() {
            anyhow::bail!("Vault root path does not exist: {}", self.config.root_path);
        }

        let mut files = Vec::new();
        self.scan_dir(root, &mut files)?;
        Ok(files)
    }

    /// 递归扫描目录
    fn scan_dir(&self, dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if self.config.recursive && !self.is_excluded(&path) {
                    self.scan_dir(&path, files)?;
                }
            } else if path.is_file() && self.is_matching_extension(&path) && !self.is_excluded(&path) {
                files.push(path);
            }
        }
        Ok(())
    }

    /// 读取文件内容
    fn read_file(&self, path: &Path) -> anyhow::Result<ContentItem> {
        let content = std::fs::read_to_string(path)?;
        let source = path.to_string_lossy().to_string();
        let title = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();

        let content_type = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| match ext {
                "md" | "markdown" => "markdown",
                "html" | "htm" => "html",
                _ => "text",
            })
            .unwrap_or("text")
            .to_string();

        let content_hash = compute_content_hash(&content);

        Ok(ContentItem {
            source,
            content_type,
            content,
            title: Some(title),
            metadata: serde_json::json!({
                "content_hash": content_hash,
                "connector": "vault",
            }),
            file_references: vec![],
            tags: vec![],
        })
    }
}

#[async_trait]
impl Connector for VaultConnector {
    fn name(&self) -> &str {
        "vault"
    }

    async fn connect(&self) -> anyhow::Result<()> {
        let root = Path::new(&self.config.root_path);
        if !root.exists() {
            anyhow::bail!("Vault root path does not exist: {}", self.config.root_path);
        }
        tracing::info!("Vault connector connected to: {}", self.config.root_path);
        Ok(())
    }

    async fn list_changes(&self, since: &openmind_core::SyncState) -> anyhow::Result<Vec<ContentChange>> {
        let files = self.scan_files()?;
        let mut changes = Vec::new();

        for file in files {
            let file_id = file.to_string_lossy().to_string();

            // Check modification time for incremental sync
            if let Ok(metadata) = std::fs::metadata(&file) {
                if let Ok(modified) = metadata.modified() {
                    let modified_time: DateTime<Utc> = modified.into();
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
        let path = Path::new(id);
        if !path.exists() {
            anyhow::bail!("File not found: {}", id);
        }
        self.read_file(path)
    }
}

#[async_trait]
impl EnhancedConnector for VaultConnector {
    fn capabilities(&self) -> ConnectorCapabilities {
        ConnectorCapabilities::new(
            vec!["markdown", "text", "html"],
            "poll",
            "incremental",
        )
        .with_capability(openmind_core::Capability::new(
            "recursive_scan",
            "Recursively scan directories for files",
        ))
        .with_capability(openmind_core::Capability::new(
            "incremental_sync",
            "Only sync files modified since last sync",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create test files
        fs::write(root.join("test1.md"), "# Test 1\n\nContent 1").unwrap();
        fs::write(root.join("test2.txt"), "Plain text content").unwrap();
        fs::write(root.join("test3.html"), "<html><body>HTML content</body></html>").unwrap();

        // Create subdirectory with file
        fs::create_dir(root.join("notes")).unwrap();
        fs::write(root.join("notes/note1.md"), "# Note 1\n\nNote content").unwrap();

        // Create excluded directory
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git/ignore.md"), "Should be ignored").unwrap();

        dir
    }

    #[tokio::test]
    async fn test_vault_scan_files() {
        let dir = setup_test_vault();
        let connector = VaultConnector::with_path(dir.path().to_string_lossy().to_string());

        let files = connector.scan_files().unwrap();
        assert!(files.len() >= 3, "Should find at least 3 matching files");

        // .git/ignore.md should be excluded
        let git_files: Vec<_> = files.iter().filter(|f| f.to_string_lossy().contains(".git")).collect();
        assert!(git_files.is_empty(), "Files in .git should be excluded");
    }

    #[tokio::test]
    async fn test_vault_fetch_content() {
        let dir = setup_test_vault();
        let connector = VaultConnector::with_path(dir.path().to_string_lossy().to_string());

        let file_path = dir.path().join("test1.md").to_string_lossy().to_string();
        let item = connector.fetch_content(&file_path).await.unwrap();
        assert_eq!(item.content_type, "markdown");
        assert!(item.title.is_some());
        assert!(item.content.contains("Test 1"));
    }

    #[tokio::test]
    async fn test_vault_connect() {
        let dir = setup_test_vault();
        let connector = VaultConnector::with_path(dir.path().to_string_lossy().to_string());

        assert!(connector.connect().await.is_ok());
    }

    #[tokio::test]
    async fn test_vault_capabilities() {
        let connector = VaultConnector::with_path("/tmp/test");
        let caps = connector.capabilities();
        assert!(caps.supported_formats.contains(&"markdown".to_string()));
        assert_eq!(caps.poll_strategy, "poll");
        assert_eq!(caps.sync_mode, "incremental");
    }

    #[tokio::test]
    async fn test_vault_health_check() {
        let dir = setup_test_vault();
        let connector = VaultConnector::with_path(dir.path().to_string_lossy().to_string());

        let health = connector.health_check().await;
        assert!(health.is_healthy);
    }
}
