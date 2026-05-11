//! 关键词搜索引擎
//!
//! 基于SQLite FTS5的全文搜索，是搜索的基础层。
//! 不依赖任何AI模型，永远可用。

use openmind_core::{KnowledgeStore, SearchFilters, SearchResult};

/// 关键词搜索引擎
///
/// 封装KnowledgeStore的query_keyword方法，
/// 提供关键词搜索、标签搜索、时间范围搜索等基础搜索能力。
pub struct KeywordSearchEngine<K: KnowledgeStore> {
    store: K,
}

impl<K: KnowledgeStore> KeywordSearchEngine<K> {
    pub fn new(store: K) -> Self {
        Self { store }
    }

    /// 关键词搜索
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> anyhow::Result<Vec<SearchResult>> {
        self.store.query_keyword(query, limit, filters).await
    }

    /// 按标签搜索
    pub async fn search_by_tags(
        &self,
        tags: &[String],
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let filters = SearchFilters {
            tags: tags.to_vec(),
            ..Default::default()
        };
        // Use a wildcard query to match all, then filter by tags
        self.store.query_keyword("*", limit, &filters).await
    }

    /// 按来源搜索
    pub async fn search_by_source(
        &self,
        source: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let filters = SearchFilters {
            source: Some(source.to_string()),
            ..Default::default()
        };
        self.store.query_keyword("*", limit, &filters).await
    }

    /// 按项目搜索
    pub async fn search_by_project(
        &self,
        project: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let filters = SearchFilters {
            project: Some(project.to_string()),
            ..Default::default()
        };
        self.store.query_keyword("*", limit, &filters).await
    }
}
