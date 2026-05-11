//! 博客文章摄入Connector
//!
//! 从博客源（RSS/本地Markdown目录）摄入文章到知识库。

use async_trait::async_trait;
use openmind_core::connector_registry::{ConnectorCapabilities, EnhancedConnector};
use openmind_core::{compute_content_hash, Connector, ContentChange, ContentItem};
use serde::{Deserialize, Serialize};

/// 博客Connector配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogConfig {
    /// 博客文章目录路径
    pub posts_path: String,
    /// 默认标签
    pub default_tags: Vec<String>,
    /// URL前缀（用于生成source_id）
    pub url_prefix: Option<String>,
}

impl Default for BlogConfig {
    fn default() -> Self {
        Self {
            posts_path: String::new(),
            default_tags: vec!["blog".to_string()],
            url_prefix: None,
        }
    }
}

/// 博客Connector
///
/// 从本地Markdown目录摄入博客文章。
/// 每篇文章视为一个知识条目，标题从frontmatter或H1提取。
pub struct BlogConnector {
    /// 配置
    config: BlogConfig,
}

impl BlogConnector {
    /// 创建新的BlogConnector
    pub fn new(config: BlogConfig) -> Self {
        Self { config }
    }

    /// 使用路径创建
    pub fn with_path(posts_path: impl Into<String>) -> Self {
        let mut config = BlogConfig::default();
        config.posts_path = posts_path.into();
        Self { config }
    }

    /// 解析Markdown frontmatter
    fn parse_frontmatter(&self, content: &str) -> (serde_json::Value, String) {
        if content.starts_with("---\n") {
            if let Some(end) = content[4..].find("\n---\n") {
                let yaml_str = &content[4..end + 4];
                let body = content[end + 8..].to_string();
                // Simple YAML-like parsing (just extract key: value pairs)
                let mut metadata = serde_json::Map::new();
                for line in yaml_str.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"');
                        if !key.is_empty() {
                            metadata.insert(
                                key.to_string(),
                                serde_json::Value::String(value.to_string()),
                            );
                        }
                    }
                }
                return (serde_json::Value::Object(metadata), body);
            }
        }
        (serde_json::json!({}), content.to_string())
    }

    /// 扫描博客文章
    fn scan_posts(&self) -> anyhow::Result<Vec<std::path::PathBuf>> {
        let root = std::path::Path::new(&self.config.posts_path);
        if !root.exists() {
            anyhow::bail!("Blog posts path does not exist: {}", self.config.posts_path);
        }

        let mut files = Vec::new();
        let entries = std::fs::read_dir(root)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "md" | "markdown") {
                    files.push(path);
                }
            }
        }
        Ok(files)
    }
}

#[async_trait]
impl Connector for BlogConnector {
    fn name(&self) -> &str {
        "blog"
    }

    async fn connect(&self) -> anyhow::Result<()> {
        let root = std::path::Path::new(&self.config.posts_path);
        if !root.exists() {
            anyhow::bail!("Blog posts path does not exist: {}", self.config.posts_path);
        }
        tracing::info!("Blog connector connected to: {}", self.config.posts_path);
        Ok(())
    }

    async fn list_changes(
        &self,
        _since: &openmind_core::SyncState,
    ) -> anyhow::Result<Vec<ContentChange>> {
        let files = self.scan_posts()?;
        Ok(files
            .into_iter()
            .map(|f| ContentChange::Added(f.to_string_lossy().to_string()))
            .collect())
    }

    async fn fetch_content(&self, id: &str) -> anyhow::Result<ContentItem> {
        let path = std::path::Path::new(id);
        if !path.exists() {
            anyhow::bail!("Blog post not found: {}", id);
        }

        let raw_content = std::fs::read_to_string(path)?;
        let (frontmatter, body) = self.parse_frontmatter(&raw_content);

        // Extract title from frontmatter or first H1
        let title = frontmatter
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                body.lines()
                    .find(|l| l.starts_with("# "))
                    .map(|l| l.trim_start_matches("# ").to_string())
            })
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("untitled")
                    .to_string()
            });

        // Extract tags from frontmatter
        let mut tags = self.config.default_tags.clone();
        if let Some(frontmatter_tags) = frontmatter.get("tags").and_then(|v| v.as_str()) {
            for tag in frontmatter_tags.split(',') {
                let tag = tag.trim().to_string();
                if !tag.is_empty() && !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }

        let content_hash = compute_content_hash(&body);

        Ok(ContentItem {
            source: id.to_string(),
            content_type: "markdown".to_string(),
            content: body.to_string(),
            title: Some(title),
            metadata: serde_json::json!({
                "frontmatter": frontmatter,
                "content_hash": content_hash,
                "connector": "blog",
            }),
            file_references: vec![],
            tags,
        })
    }
}

#[async_trait]
impl EnhancedConnector for BlogConnector {
    fn capabilities(&self) -> ConnectorCapabilities {
        ConnectorCapabilities::new(vec!["markdown"], "poll", "incremental").with_capability(
            openmind_core::Capability::new(
                "frontmatter_parsing",
                "Parse YAML frontmatter from Markdown files",
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let connector = BlogConnector::with_path("/tmp/test");
        let content = "---\ntitle: My Post\ntags: rust, programming\n---\n\n# Content here";
        let (fm, body) = connector.parse_frontmatter(content);

        assert_eq!(fm.get("title").and_then(|v| v.as_str()), Some("My Post"));
        assert!(body.contains("Content here"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let connector = BlogConnector::with_path("/tmp/test");
        let content = "# Just a heading\n\nNo frontmatter here.";
        let (fm, body) = connector.parse_frontmatter(content);

        assert!(fm.as_object().unwrap().is_empty());
        assert!(body.contains("Just a heading"));
    }

    #[tokio::test]
    async fn test_blog_fetch_content_with_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let post_path = dir.path().join("my-post.md");
        std::fs::write(
            &post_path,
            "---\ntitle: Test Post\ntags: rust, test\n---\n\n# Test Post\n\nThis is content.",
        )
        .unwrap();

        let connector = BlogConnector::with_path(dir.path().to_string_lossy().to_string());
        let item = connector
            .fetch_content(&post_path.to_string_lossy())
            .await
            .unwrap();

        assert_eq!(item.title, Some("Test Post".to_string()));
        assert!(item.tags.contains(&"rust".to_string()));
        assert!(item.tags.contains(&"blog".to_string())); // default tag
    }

    #[tokio::test]
    async fn test_blog_capabilities() {
        let connector = BlogConnector::with_path("/tmp/test");
        let caps = connector.capabilities();
        assert!(caps.supported_formats.contains(&"markdown".to_string()));
    }
}
