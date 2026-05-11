//! 通用注册表
//!
//! 为所有可扩展组件（Connector/EmbeddingModel/SearchEngine/Storage/VectorStore）
//! 提供统一的注册-发现模式。
//!
//! 设计原则：
//! - 新组件注册即可用，不改核心代码
//! - 组件声明自己的能力(capabilities)，系统自动发现和编排
//! - 配置驱动行为，功能开关/策略选择通过配置而非硬编码

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 组件能力声明
///
/// 每个组件声明自己的能力，系统据此自动发现和编排。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// 能力名称
    pub name: String,
    /// 能力描述
    pub description: String,
    /// 能力参数（如支持的内容类型、最大并发数等）
    pub params: HashMap<String, serde_json::Value>,
}

impl Capability {
    /// 创建新能力
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            params: HashMap::new(),
        }
    }

    /// 添加参数
    pub fn with_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }
}

/// 组件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentType {
    Connector,
    EmbeddingModel,
    SearchEngine,
    Storage,
    VectorStore,
}

impl ComponentType {
    /// 类型名称
    pub fn as_str(&self) -> &str {
        match self {
            Self::Connector => "connector",
            Self::EmbeddingModel => "embedding_model",
            Self::SearchEngine => "search_engine",
            Self::Storage => "storage",
            Self::VectorStore => "vector_store",
        }
    }
}

/// 组件描述
///
/// 注册到统一注册表中的组件元信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDescriptor {
    /// 组件名称
    pub name: String,
    /// 组件类型
    pub component_type: ComponentType,
    /// 组件版本
    pub version: String,
    /// 组件描述
    pub description: String,
    /// 能力列表
    pub capabilities: Vec<Capability>,
    /// 是否为默认组件
    pub is_default: bool,
}

/// 统一注册表
///
/// 管理所有组件的注册、发现和编排。
/// 每种类型的组件可以有多个实现，其中一个被标记为默认。
pub struct UnifiedRegistry {
    components: HashMap<String, ComponentDescriptor>,
    defaults: HashMap<ComponentType, String>,
}

impl UnifiedRegistry {
    /// 创建空的统一注册表
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            defaults: HashMap::new(),
        }
    }

    /// 注册组件
    pub fn register(&mut self, descriptor: ComponentDescriptor) {
        if descriptor.is_default {
            self.defaults
                .insert(descriptor.component_type, descriptor.name.clone());
        }
        self.components.insert(descriptor.name.clone(), descriptor);
    }

    /// 获取组件描述
    pub fn get(&self, name: &str) -> Option<&ComponentDescriptor> {
        self.components.get(name)
    }

    /// 获取指定类型的默认组件
    pub fn default_for_type(&self, component_type: ComponentType) -> Option<&ComponentDescriptor> {
        self.defaults
            .get(&component_type)
            .and_then(|name| self.components.get(name))
    }

    /// 列出指定类型的所有组件
    pub fn list_by_type(&self, component_type: ComponentType) -> Vec<&ComponentDescriptor> {
        self.components
            .values()
            .filter(|d| d.component_type == component_type)
            .collect()
    }

    /// 按能力搜索组件
    pub fn find_by_capability(&self, capability_name: &str) -> Vec<&ComponentDescriptor> {
        self.components
            .values()
            .filter(|d| d.capabilities.iter().any(|c| c.name == capability_name))
            .collect()
    }

    /// 列出所有已注册组件
    pub fn list_all(&self) -> Vec<&ComponentDescriptor> {
        self.components.values().collect()
    }
}

impl Default for UnifiedRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = UnifiedRegistry::new();

        let desc = ComponentDescriptor {
            name: "qdrant".to_string(),
            component_type: ComponentType::VectorStore,
            version: "1.0".to_string(),
            description: "Qdrant vector store".to_string(),
            capabilities: vec![Capability::new("vector_search", "Vector similarity search")],
            is_default: true,
        };

        registry.register(desc);
        assert!(registry.get("qdrant").is_some());
        assert!(registry
            .default_for_type(ComponentType::VectorStore)
            .is_some());
    }

    #[test]
    fn test_find_by_capability() {
        let mut registry = UnifiedRegistry::new();

        registry.register(ComponentDescriptor {
            name: "vault".to_string(),
            component_type: ComponentType::Connector,
            version: "1.0".to_string(),
            description: "OpenVault connector".to_string(),
            capabilities: vec![
                Capability::new("file_sync", "File synchronization"),
                Capability::new("markdown_parse", "Markdown parsing"),
            ],
            is_default: false,
        });

        registry.register(ComponentDescriptor {
            name: "blog".to_string(),
            component_type: ComponentType::Connector,
            version: "1.0".to_string(),
            description: "Blog connector".to_string(),
            capabilities: vec![Capability::new("markdown_parse", "Markdown parsing")],
            is_default: false,
        });

        let results = registry.find_by_capability("markdown_parse");
        assert_eq!(results.len(), 2);

        let results = registry.find_by_capability("file_sync");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_list_by_type() {
        let mut registry = UnifiedRegistry::new();

        registry.register(ComponentDescriptor {
            name: "vault".to_string(),
            component_type: ComponentType::Connector,
            version: "1.0".to_string(),
            description: "Vault".to_string(),
            capabilities: vec![],
            is_default: true,
        });

        registry.register(ComponentDescriptor {
            name: "in_memory".to_string(),
            component_type: ComponentType::VectorStore,
            version: "1.0".to_string(),
            description: "In memory".to_string(),
            capabilities: vec![],
            is_default: true,
        });

        assert_eq!(registry.list_by_type(ComponentType::Connector).len(), 1);
        assert_eq!(registry.list_by_type(ComponentType::VectorStore).len(), 1);
    }
}
