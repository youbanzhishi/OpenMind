//! Action注册表
//!
//! 管理所有已注册的Action，支持按名称查找和执行。
//! 集成中间件链，所有Action执行自动经过中间件。

use crate::composite::CompositeAction;
use crate::middleware::MiddlewareChain;
use crate::protocol::{
    Action, ActionContext, ActionInput, ActionOutput, ActionResult, ActionSchema,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

/// Action注册表
pub struct ActionRegistry {
    actions: Mutex<HashMap<String, Box<dyn Action>>>,
    composite_actions: Mutex<HashMap<String, Box<dyn CompositeAction>>>,
    middleware_chain: MiddlewareChain,
}

impl ActionRegistry {
    pub fn new(middleware_chain: MiddlewareChain) -> Self {
        Self {
            actions: Mutex::new(HashMap::new()),
            composite_actions: Mutex::new(HashMap::new()),
            middleware_chain,
        }
    }

    /// 注册Action
    pub fn register(&self, action: Box<dyn Action>) {
        let mut actions = self.actions.lock().unwrap();
        let name = action.name().to_string();
        actions.insert(name, action);
    }

    /// 注册组合Action
    pub fn register_composite(&self, action: Box<dyn CompositeAction>) {
        let mut actions = self.composite_actions.lock().unwrap();
        let name = action.composite_name().to_string();
        actions.insert(name, action);
    }

    /// 列出所有Action的Schema
    pub fn list_schemas(&self) -> Vec<ActionSchema> {
        let actions = self.actions.lock().unwrap();
        actions.values().map(|a| a.schema().clone()).collect()
    }

    /// 获取Action Schema
    pub fn get_schema(&self, name: &str) -> Option<ActionSchema> {
        let actions = self.actions.lock().unwrap();
        actions.get(name).map(|a| a.schema().clone())
    }

    /// 执行Action（经过中间件链）
    ///
    /// 为了避免MutexGuard跨await导致Send问题，
    /// 采用take-execute-putback模式。
    pub async fn execute(
        &self,
        action_name: &str,
        input: ActionInput,
        context: ActionContext,
    ) -> ActionResult {
        // Step 1: Validate input (sync, lock released after)
        {
            let actions = self.actions.lock().unwrap();
            match actions.get(action_name) {
                Some(action) => {
                    if let Err(e) = action.schema().validate_input(&input) {
                        return ActionResult {
                            output: ActionOutput::validation_err(e),
                            middleware_trace: vec![],
                        };
                    }
                }
                None => {
                    return ActionResult {
                        output: ActionOutput::err(format!("Action '{}' not found", action_name)),
                        middleware_trace: vec![],
                    };
                }
            }
        }

        // Step 2: Take action out, execute, put back
        let action_box = {
            let mut actions = self.actions.lock().unwrap();
            actions.remove(action_name)
        };

        match action_box {
            Some(action) => {
                let result = self.middleware_chain.execute(action.as_ref(), input, context).await;

                // Put the action back
                {
                    let mut actions = self.actions.lock().unwrap();
                    actions.insert(action_name.to_string(), action);
                }

                result
            }
            None => ActionResult {
                output: ActionOutput::err(format!("Action '{}' not found", action_name)),
                middleware_trace: vec![],
            },
        }
    }
}

/// Action注册表的ActionExecutor实现（用于组合Action内部调用）
pub struct RegistryExecutor {
    registry: std::sync::Arc<ActionRegistry>,
}

impl RegistryExecutor {
    pub fn new(registry: std::sync::Arc<ActionRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl crate::composite::ActionExecutor for RegistryExecutor {
    async fn execute_action(
        &self,
        action_name: &str,
        input: ActionInput,
        context: ActionContext,
    ) -> ActionOutput {
        let result = self.registry.execute(action_name, input, context).await;
        result.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SchemaField;
    use serde_json::json;

    struct TestAction;

    #[async_trait]
    impl Action for TestAction {
        fn name(&self) -> &str { "test_action" }
        fn schema(&self) -> &ActionSchema {
            static SCHEMA: once_cell::sync::Lazy<ActionSchema> = once_cell::sync::Lazy::new(|| {
                ActionSchema {
                    name: "test_action".to_string(),
                    description: "A test action".to_string(),
                    input_fields: vec![SchemaField::new("input", "string").required()],
                    output_fields: vec![SchemaField::new("output", "string").required()],
                    requires_auth: false,
                    rate_limit: 0,
                }
            });
            &SCHEMA
        }
        async fn execute(&self, input: ActionInput, _context: ActionContext) -> ActionOutput {
            let val = input.get_param("input")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            ActionOutput::ok(json!({"output": val}))
        }
    }

    #[tokio::test]
    async fn test_registry_register_and_execute() {
        let chain = MiddlewareChain::new();
        let registry = ActionRegistry::new(chain);
        registry.register(Box::new(TestAction));

        let input = ActionInput::new("test_action", json!({"input": "hello"}));
        let ctx = ActionContext::new();
        let result = registry.execute("test_action", input, ctx).await;
        assert_eq!(result.output.status, crate::protocol::ActionStatus::Success);
    }

    #[tokio::test]
    async fn test_registry_action_not_found() {
        let chain = MiddlewareChain::new();
        let registry = ActionRegistry::new(chain);

        let input = ActionInput::new("nonexistent", json!({}));
        let ctx = ActionContext::new();
        let result = registry.execute("nonexistent", input, ctx).await;
        assert_eq!(result.output.status, crate::protocol::ActionStatus::Failed);
    }

    #[tokio::test]
    async fn test_registry_validation_error() {
        let chain = MiddlewareChain::new();
        let registry = ActionRegistry::new(chain);
        registry.register(Box::new(TestAction));

        let input = ActionInput::new("test_action", json!({})); // Missing required "input"
        let ctx = ActionContext::new();
        let result = registry.execute("test_action", input, ctx).await;
        assert_eq!(result.output.status, crate::protocol::ActionStatus::ValidationError);
    }

    #[test]
    fn test_list_schemas() {
        let chain = MiddlewareChain::new();
        let registry = ActionRegistry::new(chain);
        registry.register(Box::new(TestAction));

        let schemas = registry.list_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].name, "test_action");
    }
}
