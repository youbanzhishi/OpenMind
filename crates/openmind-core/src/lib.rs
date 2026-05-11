//! OpenMind Core - 核心抽象与数据模型
//!
//! 定义了知识引擎的核心trait和基础数据结构：
//! - `Connector` trait: 数据源可插拔接口
//! - `StorageBackend` trait: 存储后端可插拔
//! - `EmbeddingModel` trait: 嵌入模型可插拔
//! - `KnowledgeStore` trait: 知识存储接口
//! - `IngestionPipeline` trait: 摄入管道

pub mod models;
pub mod traits;
pub mod sqlite_store;
pub mod embedding;

pub use models::*;
pub use traits::*;
pub use sqlite_store::{SqliteKnowledgeStore, compute_content_hash};
pub use embedding::{OpenAIEmbeddingModel, DummyEmbeddingModel};
