//! 应用状态


/// 应用共享状态
///
/// 通过Axum的State机制注入到各路由处理器。
pub struct AppState {
    /// 服务版本
    pub version: String,
    /// 已注册的Connector列表
    pub connectors: Vec<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            connectors: Vec::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
