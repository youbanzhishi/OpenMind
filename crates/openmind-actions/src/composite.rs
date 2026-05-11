//! 组合Action
//!
//! 支持将多个Action组合为一个新的复合Action，
//! 实现工作流编排（如search_and_mix）。

use crate::protocol::{Action, ActionContext, ActionInput, ActionOutput, ActionSchema, SchemaField};
use async_trait::async_trait;
use serde_json::json;


/// 组合Action trait
///
/// 定义如何将多个步骤串联为一个Action。
#[async_trait]
pub trait CompositeAction: Send + Sync {
    /// 组合Action名称
    fn composite_name(&self) -> &str;

    /// 执行组合逻辑
    async fn execute_composite(
        &self,
        input: ActionInput,
        context: ActionContext,
        executor: &dyn ActionExecutor,
    ) -> ActionOutput;
}

/// Action执行器（用于组合Action内部调用其他Action）
#[async_trait]
pub trait ActionExecutor: Send + Sync {
    /// 执行指定Action
    async fn execute_action(
        &self,
        action_name: &str,
        input: ActionInput,
        context: ActionContext,
    ) -> ActionOutput;
}

/// SearchAndMix - 组合Action示例
///
/// 先搜索知识库，再混合关键词和语义结果。
/// 演示了Agent间调用：search(hybrid) → 结果融合
pub struct SearchAndMixAction {
    schema: ActionSchema,
}

impl SearchAndMixAction {
    pub fn new() -> Self {
        Self {
            schema: ActionSchema {
                name: "search_and_mix".to_string(),
                description: "混合搜索：先关键词搜索，再语义搜索，合并去重".to_string(),
                input_fields: vec![
                    SchemaField::new("query", "string").required().description("搜索查询"),
                    SchemaField::new("limit", "integer").description("每类最大结果数"),
                ],
                output_fields: vec![
                    SchemaField::new("keyword_results", "array").required().description("关键词搜索结果"),
                    SchemaField::new("semantic_results", "array").required().description("语义搜索结果"),
                    SchemaField::new("merged_results", "array").required().description("合并去重结果"),
                ],
                requires_auth: false,
                rate_limit: 30,
            },
        }
    }
}

impl Default for SearchAndMixAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Action for SearchAndMixAction {
    fn name(&self) -> &str {
        "search_and_mix"
    }

    fn schema(&self) -> &ActionSchema {
        &self.schema
    }

    async fn execute(&self, input: ActionInput, _context: ActionContext) -> ActionOutput {
        let query = input.get_param("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if query.is_empty() {
            return ActionOutput::validation_err("query is required");
        }

        let limit = input.get_param("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // In a real implementation, this would call search actions via executor
        // For now, return structured output showing the composition pattern
        ActionOutput::ok(json!({
            "keyword_results": [],
            "semantic_results": [],
            "merged_results": [],
            "query": query,
            "limit": limit,
            "note": "Composite action: search_and_mix delegates to keyword_search + semantic_search"
        }))
    }
}

#[async_trait]
impl CompositeAction for SearchAndMixAction {
    fn composite_name(&self) -> &str {
        "search_and_mix"
    }

    async fn execute_composite(
        &self,
        input: ActionInput,
        context: ActionContext,
        executor: &dyn ActionExecutor,
    ) -> ActionOutput {
        let query = input.get_param("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if query.is_empty() {
            return ActionOutput::validation_err("query is required");
        }

        let limit = input.get_param("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // Step 1: Keyword search
        let keyword_input = ActionInput::new("search", json!({
            "query": query,
            "mode": "keyword",
            "limit": limit
        }));
        let keyword_output = executor.execute_action("search", keyword_input, context.clone()).await;

        // Step 2: Semantic search
        let semantic_input = ActionInput::new("search", json!({
            "query": query,
            "mode": "semantic",
            "limit": limit
        }));
        let semantic_output = executor.execute_action("search", semantic_input, context.clone()).await;

        // Step 3: Merge results
        let keyword_data = keyword_output.data;
        let semantic_data = semantic_output.data;

        // Simple merge: concatenate and dedupe by entry ID
        let merged = merge_search_results(&keyword_data, &semantic_data);

        ActionOutput::ok(json!({
            "keyword_results": keyword_data,
            "semantic_results": semantic_data,
            "merged_results": merged
        }))
    }
}

/// 合并搜索结果（简单去重）
fn merge_search_results(keyword_data: &serde_json::Value, semantic_data: &serde_json::Value) -> serde_json::Value {
    let mut seen_ids = std::collections::HashSet::new();
    let mut merged = Vec::new();

    for data in [keyword_data, semantic_data] {
        if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
            for item in results {
                if let Some(id) = item.get("entry").and_then(|e| e.get("id")).and_then(|i| i.as_str()) {
                    if seen_ids.insert(id.to_string()) {
                        merged.push(item.clone());
                    }
                } else {
                    merged.push(item.clone());
                }
            }
        }
    }

    serde_json::Value::Array(merged)
}

/// IngestAndRelate - 另一个组合Action示例
///
/// 摄入内容后自动建立关联。
pub struct IngestAndRelateAction {
    schema: ActionSchema,
}

impl IngestAndRelateAction {
    pub fn new() -> Self {
        Self {
            schema: ActionSchema {
                name: "ingest_and_relate".to_string(),
                description: "摄入内容并自动建立知识关联".to_string(),
                input_fields: vec![
                    SchemaField::new("source", "string").required().description("内容来源"),
                    SchemaField::new("content", "string").required().description("文本内容"),
                    SchemaField::new("auto_relate", "boolean").description("是否自动关联"),
                ],
                output_fields: vec![
                    SchemaField::new("entry_id", "string").required(),
                    SchemaField::new("relations_created", "integer").required(),
                ],
                requires_auth: true,
                rate_limit: 10,
            },
        }
    }
}

impl Default for IngestAndRelateAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Action for IngestAndRelateAction {
    fn name(&self) -> &str {
        "ingest_and_relate"
    }

    fn schema(&self) -> &ActionSchema {
        &self.schema
    }

    async fn execute(&self, input: ActionInput, _context: ActionContext) -> ActionOutput {
        let source = input.get_param("source")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let content = input.get_param("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if source.is_empty() || content.is_empty() {
            return ActionOutput::validation_err("source and content are required");
        }

        let auto_relate = input.get_param("auto_relate")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Placeholder: In real impl, would call ingest then get_related
        ActionOutput::ok(json!({
            "entry_id": uuid::Uuid::new_v4().to_string(),
            "relations_created": if auto_relate { 3 } else { 0 },
            "source": source,
            "content_length": content.len(),
            "auto_relate": auto_relate
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_and_mix_action() {
        let action = SearchAndMixAction::new();
        let input = ActionInput::new("search_and_mix", json!({
            "query": "Rust programming",
            "limit": 5
        }));
        let ctx = ActionContext::new();

        let output = action.execute(input, ctx).await;
        assert_eq!(output.status, ActionOutput::ok(json!({})).status);
        assert!(output.data.get("query").is_some());
    }

    #[tokio::test]
    async fn test_search_and_mix_missing_query() {
        let action = SearchAndMixAction::new();
        let input = ActionInput::new("search_and_mix", json!({}));
        let ctx = ActionContext::new();

        let output = action.execute(input, ctx).await;
        assert_eq!(output.status, crate::protocol::ActionStatus::ValidationError);
    }

    #[tokio::test]
    async fn test_ingest_and_relate_action() {
        let action = IngestAndRelateAction::new();
        let input = ActionInput::new("ingest_and_relate", json!({
            "source": "test.md",
            "content": "Hello world"
        }));
        let ctx = ActionContext::new();

        let output = action.execute(input, ctx).await;
        assert_eq!(output.status, ActionOutput::ok(json!({})).status);
        assert!(output.data.get("entry_id").is_some());
    }

    #[test]
    fn test_merge_search_results() {
        let keyword = json!({
            "results": [
                {"entry": {"id": "1", "title": "A"}},
                {"entry": {"id": "2", "title": "B"}}
            ]
        });
        let semantic = json!({
            "results": [
                {"entry": {"id": "2", "title": "B"}},
                {"entry": {"id": "3", "title": "C"}}
            ]
        });

        let merged = merge_search_results(&keyword, &semantic);
        let arr = merged.as_array().unwrap();
        assert_eq!(arr.len(), 3); // Deduped: 1, 2, 3
    }
}
