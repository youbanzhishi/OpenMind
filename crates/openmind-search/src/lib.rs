//! OpenMind Search - 搜索引擎
//!
//! 支持关键词搜索（FTS5）、语义搜索和混合搜索。
//! 核心原则：关键词搜索是基础权利，永远可用。
//!
//! 模块：
//! - `keyword`: 关键词搜索引擎（基于FTS5）
//! - `semantic`: 语义搜索引擎（基于向量相似度）
//! - `hybrid`: 混合搜索引擎（关键词+语义融合）
//! - `rag`: RAG查询管道（检索→重排→生成）

pub mod hybrid;
pub mod keyword;
pub mod rag;
pub mod semantic;

pub use hybrid::HybridSearchEngine;
pub use keyword::KeywordSearchEngine;
pub use rag::{RagContext, RagPipeline, RagRequest, RagResponse, RerankStrategy};
pub use semantic::SemanticSearchEngine;
