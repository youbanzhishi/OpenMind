//! OpenMind Ingest - 摄入管道
//!
//! 将原始内容转化为可搜索的知识条目：
//! 解析 → 分块 → 嵌入 → 索引 → 关联

pub mod pipeline;

pub use pipeline::DefaultIngestionPipeline;
