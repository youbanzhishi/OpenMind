//! Action Protocol - 输入输出契约定义
//!
//! 每个Action声明自己的输入/输出Schema（JSON Schema风格），
//! 执行前自动校验输入，执行后自动校验输出。
//! 这确保Agent间调用的类型安全。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Action输入
///
/// 包装了具体的输入参数和执行上下文。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInput {
    /// Action名称
    pub action: String,
    /// 输入参数（JSON对象）
    pub params: Value,
    /// 上下文元数据（调用者、请求ID等）
    pub context: HashMap<String, String>,
}

impl ActionInput {
    /// 创建新的Action输入
    pub fn new(action: impl Into<String>, params: Value) -> Self {
        Self {
            action: action.into(),
            params,
            context: HashMap::new(),
        }
    }

    /// 添加上下文
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// 获取参数中的字段
    pub fn get_param(&self, key: &str) -> Option<&Value> {
        self.params.get(key)
    }
}

/// Action输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOutput {
    /// 执行状态
    pub status: ActionStatus,
    /// 输出数据
    pub data: Value,
    /// 错误信息（如果失败）
    pub error: Option<String>,
    /// 执行耗时（毫秒）
    pub duration_ms: Option<u64>,
}

impl ActionOutput {
    /// 创建成功输出
    pub fn ok(data: Value) -> Self {
        Self {
            status: ActionStatus::Success,
            data,
            error: None,
            duration_ms: None,
        }
    }

    /// 创建失败输出
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            status: ActionStatus::Failed,
            data: Value::Null,
            error: Some(error.into()),
            duration_ms: None,
        }
    }

    /// 创建校验失败输出
    pub fn validation_err(error: impl Into<String>) -> Self {
        Self {
            status: ActionStatus::ValidationError,
            data: Value::Null,
            error: Some(error.into()),
            duration_ms: None,
        }
    }

    /// 设置耗时
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }
}

/// Action执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Success,
    Failed,
    ValidationError,
}

/// Action执行结果（包含中间件处理信息）
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// 最终输出
    pub output: ActionOutput,
    /// 经过的中间件名称
    pub middleware_trace: Vec<String>,
}

impl ActionResult {
    pub fn from_output(output: ActionOutput) -> Self {
        Self {
            output,
            middleware_trace: Vec::new(),
        }
    }
}

/// JSON Schema风格的字段描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    /// 字段名
    pub name: String,
    /// 字段类型
    #[serde(rename = "type")]
    pub field_type: String,
    /// 是否必填
    #[serde(default)]
    pub required: bool,
    /// 字段描述
    #[serde(default)]
    pub description: String,
}

impl SchemaField {
    pub fn new(name: impl Into<String>, field_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field_type: field_type.into(),
            required: false,
            description: String::new(),
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}

/// Action Schema - 输入输出契约
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSchema {
    /// Action名称
    pub name: String,
    /// Action描述
    pub description: String,
    /// 输入字段
    pub input_fields: Vec<SchemaField>,
    /// 输出字段
    pub output_fields: Vec<SchemaField>,
    /// 是否需要认证
    #[serde(default)]
    pub requires_auth: bool,
    /// 限流（每分钟请求数，0=不限）
    #[serde(default)]
    pub rate_limit: u32,
}

impl ActionSchema {
    /// 校验输入参数
    pub fn validate_input(&self, input: &ActionInput) -> Result<(), String> {
        let params = &input.params;
        if !params.is_object() {
            return Err("Input params must be a JSON object".to_string());
        }

        for field in &self.input_fields {
            if field.required {
                match params.get(&field.name) {
                    None => return Err(format!("Required field '{}' is missing", field.name)),
                    Some(Value::Null) => return Err(format!("Required field '{}' is null", field.name)),
                    Some(v) => {
                        if !validate_type(v, &field.field_type) {
                            return Err(format!(
                                "Field '{}' expected type '{}', got '{}'",
                                field.name,
                                field.field_type,
                                json_type_name(v)
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 校验输出数据
    pub fn validate_output(&self, output: &ActionOutput) -> Result<(), String> {
        if output.status != ActionStatus::Success {
            return Ok(()); // Skip validation for failed outputs
        }

        let data = &output.data;
        if !data.is_object() && !data.is_array() {
            return Ok(()); // Allow any non-object/array for flexibility
        }

        for field in &self.output_fields {
            if field.required {
                match data.get(&field.name) {
                    None => return Err(format!("Required output field '{}' is missing", field.name)),
                    Some(Value::Null) => return Err(format!("Required output field '{}' is null", field.name)),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

fn validate_type(value: &Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "any" => true,
        _ => true, // Unknown types pass
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Action执行上下文
#[derive(Debug, Clone)]
pub struct ActionContext {
    /// 请求ID
    pub request_id: String,
    /// 调用者标识
    pub caller: String,
    /// 认证令牌
    pub auth_token: Option<String>,
    /// 额外元数据
    pub metadata: HashMap<String, String>,
}

impl ActionContext {
    pub fn new() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            caller: "system".to_string(),
            auth_token: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_caller(mut self, caller: impl Into<String>) -> Self {
        self.caller = caller.into();
        self
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }
}

impl Default for ActionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Action trait - 所有Action的基础接口
#[async_trait]
pub trait Action: Send + Sync {
    /// Action名称
    fn name(&self) -> &str;

    /// Action Schema（输入输出契约）
    fn schema(&self) -> &ActionSchema;

    /// 执行Action
    async fn execute(&self, input: ActionInput, context: ActionContext) -> ActionOutput;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_schema_validation_valid_input() {
        let schema = ActionSchema {
            name: "search".to_string(),
            description: "Search knowledge".to_string(),
            input_fields: vec![
                SchemaField::new("query", "string").required().description("Search query"),
                SchemaField::new("limit", "integer").description("Max results"),
            ],
            output_fields: vec![
                SchemaField::new("results", "array").required(),
            ],
            requires_auth: false,
            rate_limit: 0,
        };

        let input = ActionInput::new("search", json!({
            "query": "Rust programming",
            "limit": 10
        }));

        assert!(schema.validate_input(&input).is_ok());
    }

    #[test]
    fn test_schema_validation_missing_required() {
        let schema = ActionSchema {
            name: "search".to_string(),
            description: "Search".to_string(),
            input_fields: vec![
                SchemaField::new("query", "string").required(),
            ],
            output_fields: vec![],
            requires_auth: false,
            rate_limit: 0,
        };

        let input = ActionInput::new("search", json!({
            "limit": 10
        }));

        let result = schema.validate_input(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("query"));
    }

    #[test]
    fn test_schema_validation_wrong_type() {
        let schema = ActionSchema {
            name: "search".to_string(),
            description: "Search".to_string(),
            input_fields: vec![
                SchemaField::new("limit", "integer").required(),
            ],
            output_fields: vec![],
            requires_auth: false,
            rate_limit: 0,
        };

        let input = ActionInput::new("search", json!({
            "limit": "not_a_number"
        }));

        let result = schema.validate_input(&input);
        assert!(result.is_err());
    }

    #[test]
    fn test_action_output_ok() {
        let output = ActionOutput::ok(json!({"id": "123"})).with_duration(42);
        assert_eq!(output.status, ActionStatus::Success);
        assert!(output.error.is_none());
        assert_eq!(output.duration_ms, Some(42));
    }

    #[test]
    fn test_action_output_err() {
        let output = ActionOutput::err("Something went wrong");
        assert_eq!(output.status, ActionStatus::Failed);
        assert_eq!(output.error.as_deref(), Some("Something went wrong"));
    }

    #[test]
    fn test_action_context() {
        let ctx = ActionContext::new()
            .with_caller("agent-1")
            .with_auth_token("secret");
        assert_eq!(ctx.caller, "agent-1");
        assert_eq!(ctx.auth_token.as_deref(), Some("secret"));
    }
}
