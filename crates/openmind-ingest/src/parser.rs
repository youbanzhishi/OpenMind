//! 内容解析器
//!
//! 支持多种格式的文本提取：
//! - Markdown: pulldown-cmark解析，保留heading层级
//! - 纯文本: 直接提取
//! - HTML: scraper提取正文内容

use async_trait::async_trait;
use openmind_core::ContentItem;
use serde_json;

/// 内容解析器trait
#[async_trait]
pub trait ContentParser: Send + Sync {
    /// 能否处理此内容类型
    fn can_parse(&self, content_type: &str) -> bool;

    /// 解析原始内容为ContentItem
    async fn parse(
        &self,
        source: &str,
        content: &str,
        content_type: &str,
    ) -> anyhow::Result<ContentItem>;
}

/// 解析器注册表
pub struct ParserRegistry {
    parsers: Vec<Box<dyn ContentParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            parsers: Vec::new(),
        };
        registry.register(Box::new(MarkdownParser));
        registry.register(Box::new(PlainTextParser));
        registry.register(Box::new(HtmlParser));
        registry
    }

    pub fn register(&mut self, parser: Box<dyn ContentParser>) {
        self.parsers.push(parser);
    }

    pub async fn parse(
        &self,
        source: &str,
        content: &str,
        content_type: &str,
    ) -> anyhow::Result<ContentItem> {
        for parser in &self.parsers {
            if parser.can_parse(content_type) {
                return parser.parse(source, content, content_type).await;
            }
        }
        // Fallback: treat as plain text
        PlainTextParser.parse(source, content, content_type).await
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Markdown解析器
pub struct MarkdownParser;

#[async_trait]
impl ContentParser for MarkdownParser {
    fn can_parse(&self, content_type: &str) -> bool {
        matches!(
            content_type,
            "markdown" | "text/markdown" | "text/x-markdown"
        )
    }

    async fn parse(
        &self,
        source: &str,
        content: &str,
        _content_type: &str,
    ) -> anyhow::Result<ContentItem> {
        use pulldown_cmark::{Event, Parser, Tag, TagEnd};

        let parser = Parser::new(content);
        let mut title = String::new();
        let mut text_parts: Vec<String> = Vec::new();
        let mut current_text = String::new();
        let mut _in_heading = false;
        let mut heading_level = 0;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    if !current_text.trim().is_empty() {
                        text_parts.push(current_text.trim().to_string());
                    }
                    current_text.clear();
                    _in_heading = true;
                    heading_level = level as usize;
                }
                Event::End(TagEnd::Heading(_)) => {
                    if heading_level == 1 && title.is_empty() {
                        title = current_text.trim().to_string();
                    } else if !current_text.trim().is_empty() {
                        text_parts.push(format!(
                            "{}{}",
                            "#".repeat(heading_level),
                            current_text.trim()
                        ));
                    }
                    current_text.clear();
                    _in_heading = false;
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    if !current_text.trim().is_empty() {
                        text_parts.push(current_text.trim().to_string());
                    }
                    current_text.clear();
                }
                Event::End(TagEnd::CodeBlock) => {
                    if !current_text.trim().is_empty() {
                        text_parts.push(format!("```\n{}\n```", current_text.trim()));
                    }
                    current_text.clear();
                }
                Event::Text(t) => {
                    current_text.push_str(&t);
                }
                Event::Code(c) => {
                    current_text.push_str(&c);
                }
                Event::SoftBreak | Event::HardBreak => {
                    current_text.push(' ');
                }
                _ => {}
            }
        }

        if !current_text.trim().is_empty() {
            text_parts.push(current_text.trim().to_string());
        }

        let full_text = text_parts.join("\n\n");
        let title = if title.is_empty() {
            source.rsplit('/').next().unwrap_or(source).to_string()
        } else {
            title
        };

        Ok(ContentItem {
            source: source.to_string(),
            content_type: "markdown".to_string(),
            content: full_text,
            title: Some(title),
            metadata: serde_json::json!({}),
            file_references: vec![],
            tags: vec![],
        })
    }
}

/// 纯文本解析器
pub struct PlainTextParser;

#[async_trait]
impl ContentParser for PlainTextParser {
    fn can_parse(&self, content_type: &str) -> bool {
        matches!(content_type, "text" | "text/plain" | "plain")
    }

    async fn parse(
        &self,
        source: &str,
        content: &str,
        _content_type: &str,
    ) -> anyhow::Result<ContentItem> {
        let title = source.rsplit('/').next().unwrap_or(source).to_string();
        Ok(ContentItem {
            source: source.to_string(),
            content_type: "text".to_string(),
            content: content.to_string(),
            title: Some(title),
            metadata: serde_json::json!({}),
            file_references: vec![],
            tags: vec![],
        })
    }
}

/// HTML解析器
pub struct HtmlParser;

#[async_trait]
impl ContentParser for HtmlParser {
    fn can_parse(&self, content_type: &str) -> bool {
        matches!(content_type, "html" | "text/html")
    }

    async fn parse(
        &self,
        source: &str,
        content: &str,
        _content_type: &str,
    ) -> anyhow::Result<ContentItem> {
        use scraper::Html;

        let document = Html::parse_document(content);

        // Extract title
        let title = document
            .select(&scraper::Selector::parse("title").unwrap())
            .next()
            .map(|el| el.inner_html().trim().to_string())
            .unwrap_or_else(|| source.rsplit('/').next().unwrap_or(source).to_string());

        // Extract body text
        let mut text_parts: Vec<String> = Vec::new();

        // Try to select body content
        let body_selector = scraper::Selector::parse("body").unwrap();
        if let Some(body) = document.select(&body_selector).next() {
            // Select headings and paragraphs
            for selector_name in &[
                "h1",
                "h2",
                "h3",
                "h4",
                "h5",
                "h6",
                "p",
                "li",
                "blockquote",
                "pre",
            ] {
                if let Ok(sel) = scraper::Selector::parse(selector_name) {
                    for el in body.select(&sel) {
                        let text = el.inner_html();
                        // Strip HTML tags for plain text
                        let plain = strip_html_tags(&text);
                        if !plain.trim().is_empty() {
                            text_parts.push(plain.trim().to_string());
                        }
                    }
                }
            }
        }

        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        let text_parts: Vec<String> = text_parts
            .into_iter()
            .filter(|s| seen.insert(s.clone()))
            .collect();

        let full_text = text_parts.join("\n\n");

        Ok(ContentItem {
            source: source.to_string(),
            content_type: "html".to_string(),
            content: full_text,
            title: Some(title),
            metadata: serde_json::json!({}),
            file_references: vec![],
            tags: vec![],
        })
    }
}

/// 简单的HTML标签剥离
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_markdown_parser() {
        let parser = MarkdownParser;
        assert!(parser.can_parse("markdown"));

        let content = r#"# My Title

This is the first paragraph.

## Section Two

This is the second section with some **bold** text.
"#;

        let item = parser.parse("test.md", content, "markdown").await.unwrap();
        assert_eq!(item.title.as_deref(), Some("My Title"));
        assert!(item.content.contains("first paragraph"));
        assert!(item.content.contains("Section Two"));
    }

    #[tokio::test]
    async fn test_plain_text_parser() {
        let parser = PlainTextParser;
        assert!(parser.can_parse("text"));

        let item = parser
            .parse("notes.txt", "Hello world", "text")
            .await
            .unwrap();
        assert_eq!(item.content, "Hello world");
    }

    #[tokio::test]
    async fn test_html_parser() {
        let parser = HtmlParser;
        assert!(parser.can_parse("html"));

        let html = r#"<html><head><title>Test Page</title></head><body><p>Hello world</p><p>Second paragraph</p></body></html>"#;
        let item = parser.parse("test.html", html, "html").await.unwrap();
        assert_eq!(item.title.as_deref(), Some("Test Page"));
        assert!(item.content.contains("Hello world"));
    }

    #[tokio::test]
    async fn test_parser_registry() {
        let registry = ParserRegistry::new();
        let item = registry
            .parse("test.md", "# Hello\n\nWorld", "markdown")
            .await
            .unwrap();
        assert_eq!(item.title.as_deref(), Some("Hello"));

        let item = registry
            .parse("test.txt", "Plain text", "text")
            .await
            .unwrap();
        assert_eq!(item.content, "Plain text");
    }
}
