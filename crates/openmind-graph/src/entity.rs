//! 实体提取与关系构建
//!
//! 从知识条目中提取实体（人物/技术/概念等）和关系，
//! 构建知识图谱的节点和边。

use openmind_core::KnowledgeEntry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 实体类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntityType {
    /// 人物
    Person,
    /// 技术/工具/语言
    Technology,
    /// 概念/思想
    Concept,
    /// 组织/团队
    Organization,
    /// 项目
    Project,
    /// 地点
    Location,
    /// 事件
    Event,
    /// 自定义
    Custom(String),
}

impl EntityType {
    /// 类型名称
    pub fn as_str(&self) -> &str {
        match self {
            Self::Person => "person",
            Self::Technology => "technology",
            Self::Concept => "concept",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Location => "location",
            Self::Event => "event",
            Self::Custom(s) => s,
        }
    }
}

/// 实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// 实体ID
    pub id: String,
    /// 实体名称
    pub name: String,
    /// 实体类型
    pub entity_type: EntityType,
    /// 实体描述
    pub description: String,
    /// 来源条目ID
    pub source_entry_ids: Vec<String>,
    /// 出现次数
    pub occurrence_count: usize,
    /// 元数据
    pub metadata: serde_json::Value,
}

/// 关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    /// 关系ID
    pub id: String,
    /// 起始实体ID
    pub from_entity_id: String,
    /// 目标实体ID
    pub to_entity_id: String,
    /// 关系类型
    pub relation_type: String,
    /// 关系权重
    pub weight: f64,
    /// 来源条目ID
    pub source_entry_ids: Vec<String>,
    /// 元数据
    pub metadata: serde_json::Value,
}

/// 实体提取器
///
/// 从知识条目中提取实体和关系。
/// 当前使用基于规则的提取，未来可接入NER模型。
pub struct EntityExtractor {
    /// 已知技术关键词
    tech_keywords: Vec<&'static str>,
    /// 已知概念关键词
    concept_keywords: Vec<&'static str>,
}

impl EntityExtractor {
    /// 创建默认实体提取器
    pub fn new() -> Self {
        Self {
            tech_keywords: vec![
                "Rust",
                "Python",
                "Go",
                "JavaScript",
                "TypeScript",
                "Java",
                "C++",
                "React",
                "Vue",
                "Svelte",
                "Next.js",
                "Docker",
                "Kubernetes",
                "Terraform",
                "PostgreSQL",
                "MySQL",
                "Redis",
                "MongoDB",
                "Qdrant",
                "SQLite",
                "Axum",
                "Tokio",
                "Actix",
                "Git",
                "GitHub",
                "CI/CD",
                "REST",
                "GraphQL",
                "gRPC",
                "OAuth",
                "JWT",
                "Linux",
                "macOS",
                "Windows",
                "AWS",
                "GCP",
                "Azure",
            ],
            concept_keywords: vec![
                "微服务",
                "分布式",
                "云原生",
                "容器化",
                "DevOps",
                "机器学习",
                "深度学习",
                "NLP",
                "RAG",
                "LLM",
                "AI",
                "数据科学",
                "数据分析",
                "安全",
                "性能优化",
                "可观测性",
                "设计模式",
                "架构",
                "重构",
                "测试",
                "持续集成",
                "持续部署",
                "知识管理",
                "知识图谱",
                "向量搜索",
                "语义搜索",
                "全文搜索",
                "事件驱动",
                "消息队列",
            ],
        }
    }

    /// 从知识条目中提取实体
    pub fn extract_entities(&self, entry: &KnowledgeEntry) -> Vec<Entity> {
        let mut entities = Vec::new();
        let text = format!("{} {}", entry.title, entry.content);

        // Extract technology entities
        for keyword in &self.tech_keywords {
            if text.contains(keyword) {
                entities.push(Entity {
                    id: Uuid::new_v4().to_string(),
                    name: keyword.to_string(),
                    entity_type: EntityType::Technology,
                    description: format!("Technology: {}", keyword),
                    source_entry_ids: vec![entry.id.clone()],
                    occurrence_count: count_occurrences(&text, keyword),
                    metadata: serde_json::json!({"extracted_by": "rule_based"}),
                });
            }
        }

        // Extract concept entities
        for keyword in &self.concept_keywords {
            if text.contains(keyword) {
                entities.push(Entity {
                    id: Uuid::new_v4().to_string(),
                    name: keyword.to_string(),
                    entity_type: EntityType::Concept,
                    description: format!("Concept: {}", keyword),
                    source_entry_ids: vec![entry.id.clone()],
                    occurrence_count: count_occurrences(&text, keyword),
                    metadata: serde_json::json!({"extracted_by": "rule_based"}),
                });
            }
        }

        // Extract entities from tags
        for tag in &entry.tags {
            // Avoid duplicates from keyword extraction
            if !entities.iter().any(|e| e.name == *tag) {
                entities.push(Entity {
                    id: Uuid::new_v4().to_string(),
                    name: tag.clone(),
                    entity_type: EntityType::Custom("tag".to_string()),
                    description: format!("Tag: {}", tag),
                    source_entry_ids: vec![entry.id.clone()],
                    occurrence_count: 1,
                    metadata: serde_json::json!({"source": "tag"}),
                });
            }
        }

        entities
    }

    /// 从同一知识条目的实体间构建关系
    pub fn extract_relations(&self, entities: &[Entity], entry: &KnowledgeEntry) -> Vec<Relation> {
        let mut relations = Vec::new();

        // Co-occurrence: entities from the same entry are related
        for i in 0..entities.len() {
            for j in (i + 1)..entities.len() {
                // Only create relations between different types
                if entities[i].entity_type != entities[j].entity_type {
                    relations.push(Relation {
                        id: Uuid::new_v4().to_string(),
                        from_entity_id: entities[i].id.clone(),
                        to_entity_id: entities[j].id.clone(),
                        relation_type: "co_occurs_with".to_string(),
                        weight: 0.5,
                        source_entry_ids: vec![entry.id.clone()],
                        metadata: serde_json::json!({
                            "entry_title": entry.title,
                        }),
                    });
                }

                // Same type → related_to
                if entities[i].entity_type == entities[j].entity_type {
                    relations.push(Relation {
                        id: Uuid::new_v4().to_string(),
                        from_entity_id: entities[i].id.clone(),
                        to_entity_id: entities[j].id.clone(),
                        relation_type: "related_to".to_string(),
                        weight: 0.3,
                        source_entry_ids: vec![entry.id.clone()],
                        metadata: serde_json::json!({
                            "shared_type": entities[i].entity_type.as_str(),
                        }),
                    });
                }
            }
        }

        relations
    }
}

impl Default for EntityExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// 统计关键词在文本中出现的次数
fn count_occurrences(text: &str, keyword: &str) -> usize {
    text.matches(keyword).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use openmind_core::{compute_content_hash, EmbeddingStatus, EntryStatus, SourceType};

    fn make_entry(title: &str, content: &str, tags: Vec<&str>) -> KnowledgeEntry {
        KnowledgeEntry {
            id: uuid::Uuid::new_v4().to_string(),
            source_type: SourceType::File,
            source_id: "test.md".to_string(),
            title: title.to_string(),
            content: content.to_string(),
            content_hash: compute_content_hash(content),
            embedding_id: None,
            embedding_status: EmbeddingStatus::Pending,
            tags: tags.into_iter().map(String::from).collect(),
            project: None,
            metadata: serde_json::json!({}),
            file_references: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: EntryStatus::Active,
        }
    }

    #[test]
    fn test_extract_tech_entities() {
        let extractor = EntityExtractor::new();
        let entry = make_entry(
            "Rust Programming",
            "Rust is a systems programming language. Use Axum for web servers.",
            vec!["rust"],
        );

        let entities = extractor.extract_entities(&entry);
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Rust"), "Should extract Rust");
        assert!(names.contains(&"Axum"), "Should extract Axum");
    }

    #[test]
    fn test_extract_concept_entities() {
        let extractor = EntityExtractor::new();
        let entry = make_entry(
            "AI Search",
            "RAG and 语义搜索 are important for AI applications.",
            vec![],
        );

        let entities = extractor.extract_entities(&entry);
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"RAG"), "Should extract RAG");
        assert!(names.contains(&"语义搜索"), "Should extract 语义搜索");
    }

    #[test]
    fn test_extract_relations() {
        let extractor = EntityExtractor::new();
        let entry = make_entry(
            "Rust and Docker",
            "Build Rust applications with Docker and Kubernetes.",
            vec![],
        );

        let entities = extractor.extract_entities(&entry);
        let relations = extractor.extract_relations(&entities, &entry);
        assert!(!relations.is_empty(), "Should find co-occurrence relations");
    }

    #[test]
    fn test_tag_entities() {
        let extractor = EntityExtractor::new();
        let entry = make_entry(
            "Custom Topic",
            "Some content about a custom topic.",
            vec!["custom-tag", "another-tag"],
        );

        let entities = extractor.extract_entities(&entry);
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"custom-tag"), "Should extract tag entity");
    }
}
