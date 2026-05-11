//! OpenMind Actions - Agent Action Protocol
//!
//! 定义Action的输入输出契约、中间件链和组合编排。
//! 每个Action声明输入/输出Schema，系统自动校验。
//! 支持认证/限流/日志中间件，支持组合Action（如search_and_mix）。

pub mod composite;
pub mod middleware;
pub mod protocol;
pub mod registry;

pub use composite::{CompositeAction, SearchAndMixAction};
pub use middleware::{
    ActionMiddleware, AuthMiddleware, LoggingMiddleware, MiddlewareChain, RateLimitMiddleware,
};
pub use protocol::{
    Action, ActionContext, ActionInput, ActionOutput, ActionResult, ActionSchema, ActionStatus,
};
pub use registry::ActionRegistry;
