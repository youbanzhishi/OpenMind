//! OpenMind Search - 搜索引擎
//!
//! 支持关键词搜索（FTS5）、语义搜索和混合搜索。
//! 核心原则：关键词搜索是基础权利，永远可用。

pub mod keyword;
pub mod hybrid;

pub use keyword::KeywordSearchEngine;
pub use hybrid::HybridSearchEngine;
