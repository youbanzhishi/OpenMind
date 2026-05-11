//! 数据模型定义
//!
//! OpenMind的核心数据结构，涵盖知识条目、文件引用、关联关系、同步状态和搜索结果。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 知识条目状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryStatus {
    Active,
    Archived,
    Pending,
    Error,
}

impl Default for EntryStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl EntryStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Pending => "pending",
            Self::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            "pending" => Some(Self::Pending),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// 嵌入状态 —— 追踪每条知识的向量化进度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmbeddingStatus {
    Embedded,
    Pending,
    Failed(String),
    Skipped,
}

impl Default for EmbeddingStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl EmbeddingStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Embedded => "embedded",
            Self::Pending => "pending",
            Self::Failed(_) => "failed",
            Self::Skipped => "skipped",
        }
    }

    pub fn from_str_simple(s: &str) -> Self {
        match s {
            "embedded" => Self::Embedded,
            "pending" => Self::Pending,
            "skipped" => Self::Skipped,
            other => Self::Failed(other.to_string()),
        }
    }
}

/// 数据源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    Blog,
    Vault,
    Bookmark,
    Note,
    File,
}

impl SourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Blog => "blog",
            Self::Vault => "vault",
            Self::Bookmark => "bookmark",
            Self::Note => "note",
            Self::File => "file",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "blog" => Some(Self::Blog),
            "vault" => Some(Self::Vault),
            "bookmark" => Some(Self::Bookmark),
            "note" => Some(Self::Note),
            "file" => Some(Self::File),
            _ => None,
        }
    }
}

/// 知识条目
///
/// 知识库的基本单元，包含文本内容及其元数据。
/// 文本内容直接存入数据层，大文件通过FileReference引用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    /// 唯一标识
    pub id: String,
    /// 数据源类型
    pub source_type: SourceType,
    /// 数据源中的原始ID
    pub source_id: String,
    /// 标题
    pub title: String,
    /// 文本内容
    pub content: String,
    /// 内容哈希（SHA-256，用于去重和变更检测）
    pub content_hash: String,
    /// 向量嵌入ID（在向量数据库中的引用）
    pub embedding_id: Option<String>,
    /// 嵌入状态（降级容灾关键字段）
    pub embedding_status: EmbeddingStatus,
    /// 标签
    pub tags: Vec<String>,
    /// 所属项目
    pub project: Option<String>,
    /// 元数据（自由键值对）
    pub metadata: serde_json::Value,
    /// 大文件引用列表
    pub file_references: Vec<FileReference>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 条目状态
    pub status: EntryStatus,
}

/// 大文件引用
///
/// 图片/音频/视频等大文件不直接存入知识库，
/// 只保留引用指针，原始文件在OpenVault/S3。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReference {
    /// 唯一标识
    pub id: String,
    /// 存储后端名称（如 "vault", "s3"）
    pub storage_backend: String,
    /// 文件访问URL
    pub url: String,
    /// 内容哈希（用于变更检测）
    pub content_hash: String,
    /// 媒体类型（如 "image/png", "audio/wav"）
    pub media_type: String,
    /// 提取的文本（如OCR结果、音频转写）
    pub extracted_text: Option<String>,
}

/// 知识关联
///
/// 描述知识条目之间的关系，支持带权重的有向图。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRelation {
    /// 关联ID
    pub id: String,
    /// 起始条目ID
    pub from_id: String,
    /// 目标条目ID
    pub to_id: String,
    /// 关系类型（如 "similar_to", "derived_from", "references", "contradicts", "part_of"）
    pub relation_type: String,
    /// 关系权重（0.0-1.0）
    pub weight: f64,
    /// 关联元数据
    pub metadata: serde_json::Value,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 同步状态
///
/// 记录每个数据源的同步进度，用于增量同步。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    /// 数据源标识（Connector名称）
    pub connector_name: String,
    /// 最后同步时间
    pub last_sync_at: DateTime<Utc>,
    /// 内容哈希（用于快速变更检测）
    pub content_hash: Option<String>,
    /// 同步状态
    pub status: String,
    /// 错误信息
    pub last_error: Option<String>,
    /// 同步总数
    pub total_synced: i64,
    /// 错误总数
    pub total_errors: i64,
}

/// 搜索结果
///
/// 统一的搜索结果格式，支持关键词和语义搜索。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// 匹配的知识条目
    pub entry: KnowledgeEntry,
    /// 相关度分数（0.0-1.0）
    pub relevance: f64,
    /// 高亮片段
    pub highlights: Vec<String>,
}

/// 内容变更（Connector返回的变更类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentChange {
    /// 新增内容
    Added(String),
    /// 修改内容
    Modified(String),
    /// 删除内容
    Deleted(String),
}

/// 内容条目（Connector返回的原始内容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    /// 来源标识
    pub source: String,
    /// 内容类型
    pub content_type: String,
    /// 文本内容
    pub content: String,
    /// 标题
    pub title: Option<String>,
    /// 元数据
    pub metadata: serde_json::Value,
    /// 关联的大文件
    pub file_references: Vec<FileReference>,
    /// 标签
    pub tags: Vec<String>,
}

/// 搜索请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    /// 搜索查询
    pub query: String,
    /// 搜索模式
    pub mode: SearchMode,
    /// 返回结果数量
    pub limit: Option<usize>,
    /// 过滤条件
    pub filters: SearchFilters,
}

/// 搜索模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// 关键词搜索
    Keyword,
    /// 语义搜索
    Semantic,
    /// 混合搜索（关键词+语义融合）
    Hybrid,
}

impl Default for SearchMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// 搜索过滤条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    /// 内容类型过滤
    pub content_type: Option<String>,
    /// 来源过滤
    pub source: Option<String>,
    /// 标签过滤
    pub tags: Vec<String>,
    /// 项目过滤
    pub project: Option<String>,
    /// 开始日期
    pub date_from: Option<DateTime<Utc>>,
    /// 结束日期
    pub date_to: Option<DateTime<Utc>>,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            content_type: None,
            source: None,
            tags: Vec::new(),
            project: None,
            date_from: None,
            date_to: None,
        }
    }
}

/// 搜索响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// 搜索结果
    pub results: Vec<SearchResult>,
    /// 搜索模式
    pub mode: SearchMode,
    /// 总结果数
    pub total: usize,
    /// 是否降级
    pub degraded: bool,
}

/// 摄入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestRequest {
    /// 来源
    pub source: String,
    /// 内容
    pub content: String,
    /// 元数据
    pub metadata: serde_json::Value,
    /// 标签
    pub tags: Vec<String>,
}

/// 摄入响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResponse {
    /// 创建的知识条目ID
    pub id: String,
    /// 状态
    pub status: String,
}

/// 健康检查响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// 服务状态
    pub status: String,
    /// 版本
    pub version: String,
    /// 已注册Connector列表
    pub connectors: Vec<String>,
    /// 嵌入模型状态
    pub embedding_status: String,
}

/// 知识库统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStats {
    /// 总条目数
    pub total_entries: i64,
    /// 各来源条目数
    pub by_source: serde_json::Value,
    /// 嵌入状态统计
    pub by_embedding_status: serde_json::Value,
    /// 总关联数
    pub total_relations: i64,
    /// 总标签数
    pub total_tags: i64,
}
