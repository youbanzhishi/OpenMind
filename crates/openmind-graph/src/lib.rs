//! OpenMind Graph - 知识图谱
//!
//! 构建和查询知识条目之间的关系图谱。
//! 支持实体提取、关系构建和图遍历。
//!
//! 模块：
//! - `builder`: 知识图谱构建器（基于语义相似度和标签关联）
//! - `entity`: 实体提取与关系构建
//! - `traits`: 图数据库隔离trait

pub mod builder;
pub mod entity;
pub mod traits;

pub use builder::GraphBuilder;
pub use entity::{EntityExtractor, Entity, EntityType, Relation};
pub use traits::{GraphStore, InMemoryGraphStore, GraphSearchResult};
