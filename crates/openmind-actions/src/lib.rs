//! OpenMind Actions - Agent Action Protocol
//!
//! 定义Action的输入输出契约、中间件链和组合编排。
//! 每个Action声明输入/输出Schema，系统自动校验。
//! 支持认证/限流/日志中间件，支持组合Action（如search_and_mix）。

pub mod protocol;
pub mod middleware;
pub mod composite;
pub mod registry;

pub use protocol::{Action, ActionContext, ActionInput, ActionOutput, ActionSchema, ActionResult, ActionStatus};
pub use middleware::{ActionMiddleware, AuthMiddleware, RateLimitMiddleware, LoggingMiddleware, MiddlewareChain};
pub use composite::{CompositeAction, SearchAndMixAction};
pub use registry::ActionRegistry;
