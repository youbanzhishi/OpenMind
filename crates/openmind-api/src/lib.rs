//! OpenMind API - HTTP API层
//!
//! 基于Axum的HTTP API，提供搜索、摄入、同步等REST接口，
//! 以及Agent发现协议（/.well-known/agent.json）。

pub mod routes;
pub mod state;

pub use routes::create_router;
pub use state::AppState;
