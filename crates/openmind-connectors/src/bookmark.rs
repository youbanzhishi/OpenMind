//! 书签导入Connector
//!
//! 从浏览器书签HTML文件或JSON导入书签到知识库。

use async_trait::async_trait;
use openmind_core::connector_registry::{ConnectorCapabilities, EnhancedConnector};
use openmind_core::{compute_content_hash, Connector, ContentChange, ContentItem};
use serde::{Deserialize, Serialize};

/// 书签条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkEntry {
    /// 书签URL
    pub url: String,
    /// 书签标题
    pub title: String,
    /// 添加时间
    pub added_date: Option<String>,
    /// 图标URL
    pub icon: Option<String>,
    /// 标签/文件夹
    pub folder: Option<String>,
}

/// 书签Connector配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkConfig {
    /// 书签文件路径
    pub bookmarks_path: String,
    /// 默认标签
    pub default_tags: Vec<String>,
}

impl Default for BookmarkConfig {
    fn default() -> Self {
        Self {
            bookmarks_path: String::new(),
            default_tags: vec!["bookmark".to_string()],
        }
    }
}

/// 书签Connector
///
/// 从书签HTML文件（Netscape格式）或JSON导入书签。
/// 大多数浏览器（Chrome/Firefox/Safari）都支持导出为Netscape格式。
pub struct BookmarkConnector {
    /// 配置
    config: BookmarkConfig,
}

impl BookmarkConnector {
    /// 创建新的BookmarkConnector
    pub fn new(config: BookmarkConfig) -> Self {
        Self { config }
    }

    /// 使用路径创建
    pub fn with_path(bookmarks_path: impl Into<String>) -> Self {
        let mut config = BookmarkConfig::default();
        config.bookmarks_path = bookmarks_path.into();
        Self { config }
    }

    /// 解析Netscape书签HTML文件
    pub fn parse_netscape_html(&self, content: &str) -> Vec<BookmarkEntry> {
        let mut bookmarks = Vec::new();
        let mut current_folder: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();

            // Track folder depth
            if line.starts_with("<DT><H3") {
                if let Some(end_tag) = line.find("</H3>") {
                    let before_close = &line[..end_tag];
                    if let Some(last_gt) = before_close.rfind('>') {
                        current_folder = Some(before_close[last_gt + 1..].trim().to_string());
                    }
                }
            } else if line.contains("</DL>") {
                current_folder = None;
            } else if line.starts_with("<DT><A") {
                if let Some(url) = Self::extract_attr(line, "HREF") {
                    let title = Self::extract_link_text(line);
                    let added_date = Self::extract_attr(line, "ADD_DATE");
                    bookmarks.push(BookmarkEntry {
                        url,
                        title,
                        added_date,
                        icon: Self::extract_attr(line, "ICON"),
                        folder: current_folder.clone(),
                    });
                }
            }
        }

        bookmarks
    }

    /// 解析JSON书签文件
    pub fn parse_json(&self, content: &str) -> anyhow::Result<Vec<BookmarkEntry>> {
        let bookmarks: Vec<BookmarkEntry> = serde_json::from_str(content)?;
        Ok(bookmarks)
    }

    /// 从HTML标签提取属性值
    fn extract_attr(tag: &str, attr: &str) -> Option<String> {
        let pattern = format!("{}=\"", attr);
        if let Some(start) = tag.find(&pattern) {
            let value_start = start + pattern.len();
            if let Some(end) = tag[value_start..].find('"') {
                return Some(tag[value_start..value_start + end].to_string());
            }
        }
        None
    }

    /// 提取链接文本
    fn extract_link_text(tag: &str) -> String {
        // Find the last '>' before '</A>' — this is the closing of the <A ...> tag
        if let Some(end_tag) = tag.find("</A>") {
            let before_close = &tag[..end_tag];
            if let Some(last_gt) = before_close.rfind('>') {
                return before_close[last_gt + 1..].trim().to_string();
            }
        }
        "Untitled".to_string()
    }

    /// 将书签条目转换为ContentItem
    pub fn bookmark_to_content_item(&self, entry: &BookmarkEntry) -> ContentItem {
        let mut tags = self.config.default_tags.clone();
        if let Some(folder) = &entry.folder {
            if !tags.contains(folder) {
                tags.push(folder.clone());
            }
        }

        let content = format!(
            "# {}\n\nURL: {}\n{}",
            entry.title,
            entry.url,
            entry
                .added_date
                .as_ref()
                .map(|d| format!("Added: {}", d))
                .unwrap_or_default()
        );
        let content_hash = compute_content_hash(&content);

        ContentItem {
            source: entry.url.clone(),
            content_type: "text".to_string(),
            content,
            title: Some(entry.title.clone()),
            metadata: serde_json::json!({
                "url": entry.url,
                "icon": entry.icon,
                "folder": entry.folder,
                "content_hash": content_hash,
                "connector": "bookmark",
            }),
            file_references: vec![],
            tags,
        }
    }
}

#[async_trait]
impl Connector for BookmarkConnector {
    fn name(&self) -> &str {
        "bookmark"
    }

    async fn connect(&self) -> anyhow::Result<()> {
        let path = std::path::Path::new(&self.config.bookmarks_path);
        if !path.exists() {
            tracing::warn!(
                "Bookmarks path does not exist: {} (will return empty)",
                self.config.bookmarks_path
            );
        }
        Ok(())
    }

    async fn list_changes(
        &self,
        _since: &openmind_core::SyncState,
    ) -> anyhow::Result<Vec<ContentChange>> {
        let path = std::path::Path::new(&self.config.bookmarks_path);
        if !path.exists() {
            return Ok(vec![]);
        }
        // All bookmarks treated as "added" since we don't track individual changes
        Ok(vec![ContentChange::Added(
            self.config.bookmarks_path.clone(),
        )])
    }

    async fn fetch_content(&self, _id: &str) -> anyhow::Result<ContentItem> {
        let content = std::fs::read_to_string(&self.config.bookmarks_path)?;
        let bookmarks = self.parse_netscape_html(&content);

        // Return a combined content item for all bookmarks
        let mut combined_content = String::new();
        let mut tags = self.config.default_tags.clone();

        for bookmark in &bookmarks {
            combined_content.push_str(&format!("- [{}]({})\n", bookmark.title, bookmark.url));
            if let Some(folder) = &bookmark.folder {
                if !tags.contains(folder) {
                    tags.push(folder.clone());
                }
            }
        }

        Ok(ContentItem {
            source: self.config.bookmarks_path.clone(),
            content_type: "text".to_string(),
            content: combined_content,
            title: Some(format!("Bookmarks ({})", bookmarks.len())),
            metadata: serde_json::json!({
                "bookmark_count": bookmarks.len(),
                "connector": "bookmark",
            }),
            file_references: vec![],
            tags,
        })
    }
}

#[async_trait]
impl EnhancedConnector for BookmarkConnector {
    fn capabilities(&self) -> ConnectorCapabilities {
        ConnectorCapabilities::new(vec!["html", "json"], "poll", "full")
            .with_capability(openmind_core::Capability::new(
                "netscape_html_parsing",
                "Parse Netscape bookmark HTML format",
            ))
            .with_capability(openmind_core::Capability::new(
                "json_import",
                "Import bookmarks from JSON",
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_netscape_html() {
        let html = r#"
<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><H3>Development</H3>
    <DL><p>
        <DT><A HREF="https://rust-lang.org" ADD_DATE="1234567890">Rust</A>
        <DT><A HREF="https://github.com" ADD_DATE="1234567891">GitHub</A>
    </DL><p>
    <DT><H3>News</H3>
    <DL><p>
        <DT><A HREF="https://news.ycombinator.com">Hacker News</A>
    </DL><p>
</DL><p>
"#;
        let connector = BookmarkConnector::with_path("/tmp/test");
        let bookmarks = connector.parse_netscape_html(html);

        assert_eq!(bookmarks.len(), 3);
        assert_eq!(bookmarks[0].url, "https://rust-lang.org");
        assert_eq!(bookmarks[0].title, "Rust");
        assert_eq!(bookmarks[0].folder, Some("Development".to_string()));
        assert_eq!(bookmarks[2].folder, Some("News".to_string()));
    }

    #[test]
    fn test_bookmark_to_content_item() {
        let connector = BookmarkConnector::with_path("/tmp/test");
        let entry = BookmarkEntry {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            added_date: Some("2024-01-01".to_string()),
            icon: None,
            folder: Some("TestFolder".to_string()),
        };

        let item = connector.bookmark_to_content_item(&entry);
        assert_eq!(item.title, Some("Example".to_string()));
        assert!(item.tags.contains(&"bookmark".to_string()));
        assert!(item.tags.contains(&"TestFolder".to_string()));
    }

    #[tokio::test]
    async fn test_bookmark_capabilities() {
        let connector = BookmarkConnector::with_path("/tmp/test");
        let caps = connector.capabilities();
        assert!(caps.supported_formats.contains(&"html".to_string()));
        assert_eq!(caps.sync_mode, "full");
    }
}
