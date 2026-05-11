//! OpenMind Ingest - 摄入管道
//!
//! 将原始内容转化为可搜索的知识条目：
//! 解析 → 分块 → 嵌入 → 索引 → 关联

pub mod chunker;
pub mod parser;
pub mod pipeline;

pub use chunker::{ChunkType, Chunker, ChunkingStrategy, TextChunk};
pub use parser::{ContentParser, HtmlParser, MarkdownParser, ParserRegistry, PlainTextParser};
pub use pipeline::DefaultIngestionPipeline;
