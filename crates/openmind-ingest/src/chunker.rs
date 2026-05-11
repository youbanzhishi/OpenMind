//! 分块策略
//!
//! 支持三种分块方式：
//! - 按段落分块（Markdown heading级别）
//! - 固定长度分块（512 token上限，100字重叠）
//! - 代码按函数/类分块

use serde::{Deserialize, Serialize};

/// 文本块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunk {
    /// 块ID
    pub id: String,
    /// 父条目ID（可选，在管道中设置）
    pub parent_id: String,
    /// 块内容
    pub content: String,
    /// 块索引（从0开始）
    pub index: usize,
    /// 在原文中的偏移（字符位置）
    pub offset: usize,
    /// 块类型
    pub chunk_type: ChunkType,
}

/// 块类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChunkType {
    /// 段落
    Paragraph,
    /// 标题下的内容
    Heading,
    /// 代码块
    CodeBlock,
    /// 列表
    List,
}

/// 分块策略
#[derive(Debug, Clone)]
pub enum ChunkingStrategy {
    /// 按段落/heading分块
    Paragraph,
    /// 固定长度分块
    FixedLength {
        max_chars: usize,
        overlap_chars: usize,
    },
}

impl Default for ChunkingStrategy {
    fn default() -> Self {
        Self::Paragraph
    }
}

/// 分块器
pub struct Chunker {
    strategy: ChunkingStrategy,
}

impl Chunker {
    pub fn new(strategy: ChunkingStrategy) -> Self {
        Self { strategy }
    }

    /// 使用默认策略创建分块器
    pub fn default_paragraph() -> Self {
        Self::new(ChunkingStrategy::Paragraph)
    }

    /// 创建固定长度分块器
    pub fn fixed_length(max_chars: usize, overlap_chars: usize) -> Self {
        Self::new(ChunkingStrategy::FixedLength {
            max_chars,
            overlap_chars,
        })
    }

    /// 分块
    pub fn chunk(&self, content: &str, content_type: &str, parent_id: &str) -> Vec<TextChunk> {
        match &self.strategy {
            ChunkingStrategy::Paragraph => {
                if content_type == "markdown" {
                    self.chunk_markdown(content, parent_id)
                } else if content_type == "code" {
                    self.chunk_code(content, parent_id)
                } else {
                    // Fallback to paragraph-based for plain text
                    self.chunk_by_paragraph(content, parent_id)
                }
            }
            ChunkingStrategy::FixedLength {
                max_chars,
                overlap_chars,
            } => self.chunk_fixed_length(content, parent_id, *max_chars, *overlap_chars),
        }
    }

    /// Markdown分块：按heading分块
    fn chunk_markdown(&self, content: &str, parent_id: &str) -> Vec<TextChunk> {
        use pulldown_cmark::{Event, Parser, Tag, TagEnd};

        let mut chunks = Vec::new();
        let mut current_heading = String::new();
        let mut current_parts: Vec<String> = Vec::new();
        let mut in_code_block = false;
        let mut code_buf = String::new();
        let mut offset = 0;

        let parser = Parser::new(content);

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level: _, .. }) => {
                    // Flush previous section
                    if !current_parts.is_empty() {
                        let text = current_parts.join("\n");
                        if !text.trim().is_empty() {
                            chunks.push(TextChunk {
                                id: uuid::Uuid::new_v4().to_string(),
                                parent_id: parent_id.to_string(),
                                content: text.trim().to_string(),
                                index: chunks.len(),
                                offset,
                                chunk_type: if current_heading.is_empty() {
                                    ChunkType::Paragraph
                                } else {
                                    ChunkType::Heading
                                },
                            });
                            offset += text.len();
                        }
                        current_parts.clear();
                    }
                    current_heading.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    current_parts.push(current_heading.clone());
                    current_heading.clear();
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    // Flush text before code
                    if !current_parts.is_empty() {
                        let text = current_parts.join("\n");
                        if !text.trim().is_empty() {
                            chunks.push(TextChunk {
                                id: uuid::Uuid::new_v4().to_string(),
                                parent_id: parent_id.to_string(),
                                content: text.trim().to_string(),
                                index: chunks.len(),
                                offset,
                                chunk_type: ChunkType::Heading,
                            });
                            offset += text.len();
                        }
                        current_parts.clear();
                    }
                    in_code_block = true;
                    code_buf.clear();
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    if !code_buf.trim().is_empty() {
                        chunks.push(TextChunk {
                            id: uuid::Uuid::new_v4().to_string(),
                            parent_id: parent_id.to_string(),
                            content: code_buf.trim().to_string(),
                            index: chunks.len(),
                            offset,
                            chunk_type: ChunkType::CodeBlock,
                        });
                        offset += code_buf.len();
                    }
                    code_buf.clear();
                }
                Event::Text(t) => {
                    if in_code_block {
                        code_buf.push_str(&t);
                    } else {
                        current_heading.push_str(&t);
                        current_parts.push(t.to_string());
                    }
                }
                Event::Code(c) => {
                    if in_code_block {
                        code_buf.push_str(&c);
                    } else {
                        current_parts.push(c.to_string());
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if in_code_block {
                        code_buf.push('\n');
                    }
                }
                _ => {}
            }
        }

        // Flush remaining
        if !current_parts.is_empty() {
            let text = current_parts.join("\n");
            if !text.trim().is_empty() {
                chunks.push(TextChunk {
                    id: uuid::Uuid::new_v4().to_string(),
                    parent_id: parent_id.to_string(),
                    content: text.trim().to_string(),
                    index: chunks.len(),
                    offset,
                    chunk_type: ChunkType::Paragraph,
                });
            }
        }

        chunks
    }

    /// 按段落分块（双换行分割）
    fn chunk_by_paragraph(&self, content: &str, parent_id: &str) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut offset = 0;

        for paragraph in content.split("\n\n") {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() {
                continue;
            }
            chunks.push(TextChunk {
                id: uuid::Uuid::new_v4().to_string(),
                parent_id: parent_id.to_string(),
                content: trimmed.to_string(),
                index: chunks.len(),
                offset,
                chunk_type: ChunkType::Paragraph,
            });
            offset += paragraph.len() + 2; // +2 for "\n\n"
        }

        chunks
    }

    /// 固定长度分块（带重叠）
    fn chunk_fixed_length(
        &self,
        content: &str,
        parent_id: &str,
        max_chars: usize,
        overlap_chars: usize,
    ) -> Vec<TextChunk> {
        if content.len() <= max_chars {
            return vec![TextChunk {
                id: uuid::Uuid::new_v4().to_string(),
                parent_id: parent_id.to_string(),
                content: content.to_string(),
                index: 0,
                offset: 0,
                chunk_type: ChunkType::Paragraph,
            }];
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < content.len() {
            let end = (start + max_chars).min(content.len());
            let chunk_content = content[start..end].to_string();

            chunks.push(TextChunk {
                id: uuid::Uuid::new_v4().to_string(),
                parent_id: parent_id.to_string(),
                content: chunk_content,
                index: chunks.len(),
                offset: start,
                chunk_type: ChunkType::Paragraph,
            });

            start = if end >= content.len() {
                break;
            } else {
                end.saturating_sub(overlap_chars)
            };
        }

        chunks
    }

    /// 代码分块：按函数/类分块
    fn chunk_code(&self, content: &str, parent_id: &str) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut offset = 0;

        // Simple heuristic: split on "fn ", "pub fn ", "async fn ", "struct ", "enum ", "impl ", "class ", "def "
        let lines: Vec<&str> = content.lines().collect();
        let mut current_block = Vec::new();
        let mut brace_count: i32 = 0;
        let mut block_start = 0;

        for (_i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect function/class/struct definitions
            let is_definition = trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ");

            if is_definition && !current_block.is_empty() && brace_count == 0 {
                // Flush previous block
                let block_content = current_block.join("\n");
                if !block_content.trim().is_empty() {
                    chunks.push(TextChunk {
                        id: uuid::Uuid::new_v4().to_string(),
                        parent_id: parent_id.to_string(),
                        content: block_content.trim().to_string(),
                        index: chunks.len(),
                        offset: block_start,
                        chunk_type: ChunkType::CodeBlock,
                    });
                }
                current_block.clear();
                block_start = offset;
            }

            // Track braces for Rust/C-like code
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_count += 1,
                    '}' => brace_count = brace_count.saturating_sub(1),
                    _ => {}
                }
            }

            // Track Python-style indentation
            if trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("async def ")
            {
                if !current_block.is_empty() && brace_count == 0 {
                    // Python: flush previous
                    let block_content = current_block.join("\n");
                    if !block_content.trim().is_empty() {
                        chunks.push(TextChunk {
                            id: uuid::Uuid::new_v4().to_string(),
                            parent_id: parent_id.to_string(),
                            content: block_content.trim().to_string(),
                            index: chunks.len(),
                            offset: block_start,
                            chunk_type: ChunkType::CodeBlock,
                        });
                    }
                    current_block.clear();
                    block_start = offset;
                }
            }

            current_block.push(line.to_string());
            offset += line.len() + 1; // +1 for newline
        }

        // Flush remaining
        if !current_block.is_empty() {
            let block_content = current_block.join("\n");
            if !block_content.trim().is_empty() {
                chunks.push(TextChunk {
                    id: uuid::Uuid::new_v4().to_string(),
                    parent_id: parent_id.to_string(),
                    content: block_content.trim().to_string(),
                    index: chunks.len(),
                    offset: block_start,
                    chunk_type: ChunkType::CodeBlock,
                });
            }
        }

        // If no chunks were created (no function definitions found), use fixed-length
        if chunks.is_empty() {
            return self.chunk_fixed_length(content, parent_id, 2000, 200);
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph_chunking() {
        let chunker = Chunker::default_paragraph();
        let content = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = chunker.chunk(content, "text", "parent-1");

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "First paragraph.");
        assert_eq!(chunks[1].content, "Second paragraph.");
        assert_eq!(chunks[2].content, "Third paragraph.");
    }

    #[test]
    fn test_markdown_chunking() {
        let chunker = Chunker::default_paragraph();
        let content = "# Title\n\nIntro paragraph.\n\n## Section 1\n\nSection content.\n\n```rust\nfn main() {}\n```";
        let chunks = chunker.chunk(content, "markdown", "parent-2");

        assert!(!chunks.is_empty());
        // Should have heading chunks and a code block
        let has_code = chunks.iter().any(|c| c.chunk_type == ChunkType::CodeBlock);
        assert!(has_code, "Should have at least one code block chunk");
    }

    #[test]
    fn test_fixed_length_chunking() {
        let chunker = Chunker::fixed_length(50, 10);
        let content = "A".repeat(200);
        let chunks = chunker.chunk(&content, "text", "parent-3");

        assert!(chunks.len() > 1, "Should produce multiple chunks");
        assert!(chunks[0].content.len() <= 50);
    }

    #[test]
    fn test_code_chunking() {
        let chunker = Chunker::default_paragraph();
        let content = r#"fn foo() {
    println!("hello");
}

fn bar() -> i32 {
    42
}
"#;
        let chunks = chunker.chunk(content, "code", "parent-4");
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_empty_content() {
        let chunker = Chunker::default_paragraph();
        let chunks = chunker.chunk("", "text", "parent-5");
        assert!(chunks.is_empty());
    }
}
