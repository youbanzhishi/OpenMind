//! OpenMind Core - 核心抽象与数据模型
//!
//! 定义了知识引擎的核心trait和基础数据结构：
//! - `Connector` trait: 数据源可插拔接口
//! - `StorageBackend` trait: 存储后端可插拔
//! - `EmbeddingModel` trait: 嵌入模型可插拔
//! - `KnowledgeStore` trait: 知识存储接口
//! - `IngestionPipeline` trait: 摄入管道
//! - `VectorStore` trait: 向量存储可插拔
//! - `EventBus`: 事件总线，组件间松耦合通信
//! - `UnifiedRegistry`: 统一注册表，组件发现与编排

pub mod connector_registry;
pub mod embedding;
pub mod event_bus;
pub mod models;
pub mod registry;
pub mod sqlite_store;
pub mod traits;
pub mod vector_store;

pub use embedding::{DummyEmbeddingModel, OpenAIEmbeddingModel};
pub use event_bus::{Event, EventBus};
pub use models::*;
pub use registry::{Capability, ComponentDescriptor, ComponentType, UnifiedRegistry};
pub use sqlite_store::{compute_content_hash, SqliteKnowledgeStore};
pub use traits::*;
pub use vector_store::{
    InMemoryVectorStore, VectorPoint, VectorSearchResult, VectorStore, VectorStoreRegistry,
};

pub use connector_registry::{
    ConnectorCapabilities, ConnectorHealth, ConnectorRegistry, EnhancedConnector,
};
