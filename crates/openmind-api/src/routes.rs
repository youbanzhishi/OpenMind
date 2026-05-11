//! API路由定义
//!
//! 所有HTTP路由和处理器。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use openmind_core::{
    HealthResponse, IngestRequest, IngestResponse, SearchRequest, SearchResponse,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

/// 创建API路由
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // API v1 routes
        .route("/api/v1/search", post(search))
        .route("/api/v1/ingest", post(ingest))
        .route("/api/v1/entry/:id", get(get_entry))
        .route("/api/v1/entry/:id/related", get(get_related))
        .route("/api/v1/sync/:source", post(trigger_sync))
        .route("/api/v1/connectors", get(list_connectors))
        .route("/api/v1/health", get(health))
        // Agent discovery
        .route("/.well-known/agent.json", get(agent_discovery))
        .with_state(state)
}

/// POST /api/v1/search - 搜索（keyword/semantic/hybrid）
async fn search(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, StatusCode> {
    // TODO: Implement with actual search engine
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// POST /api/v1/ingest - 摄入内容
async fn ingest(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<IngestRequest>,
) -> Result<Json<IngestResponse>, StatusCode> {
    // TODO: Implement with actual ingestion pipeline
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// GET /api/v1/entry/:id - 获取知识条目
async fn get_entry(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // TODO: Implement with actual store
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// GET /api/v1/entry/:id/related - 获取关联知识
async fn get_related(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // TODO: Implement with actual graph
    let _id = id;
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// POST /api/v1/sync/:source - 触发同步
async fn trigger_sync(
    State(_state): State<Arc<AppState>>,
    Path(source): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // TODO: Implement with actual connector
    let _ = source;
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// GET /api/v1/connectors - 列出已注册Connector
async fn list_connectors(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    Json(json!({
        "connectors": state.connectors,
    }))
}

/// GET /api/v1/health - 健康检查
async fn health(
    State(state): State<Arc<AppState>>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
        connectors: state.connectors.clone(),
    })
}

/// GET /.well-known/agent.json - Agent发现
async fn agent_discovery(
    State(_state): State<Arc<AppState>>,
) -> Json<Value> {
    Json(json!({
        "schema_version": "1.0",
        "name": "OpenMind",
        "description": "AI-native personal knowledge engine - knowledge node in Agent ecosystem",
        "version": env!("CARGO_PKG_VERSION"),
        "base_url": "http://localhost:9090",
        "capabilities": [
            {
                "name": "semantic_search",
                "description": "语义搜索知识库",
                "endpoint": "POST /api/v1/search",
                "input": {
                    "query": "string",
                    "mode": "keyword|semantic|hybrid",
                    "limit": 10,
                    "filters": {}
                },
                "output": {
                    "results": [{
                        "content": "string",
                        "source": "string",
                        "relevance": 0.0,
                        "highlights": []
                    }]
                }
            },
            {
                "name": "find_todos",
                "description": "查找待办事项及关联文件",
                "endpoint": "POST /api/v1/search",
                "input": {
                    "query": "string",
                    "filters": { "type": "todo" }
                },
                "output": {
                    "results": [{
                        "content": "string",
                        "files": ["string"],
                        "project": "string"
                    }]
                }
            },
            {
                "name": "ingest",
                "description": "摄入内容到知识库",
                "endpoint": "POST /api/v1/ingest",
                "input": {
                    "source": "string",
                    "content": "string",
                    "metadata": {}
                },
                "output": {
                    "id": "string",
                    "status": "indexed"
                }
            },
            {
                "name": "get_related",
                "description": "获取知识的关联条目",
                "endpoint": "GET /api/v1/entry/{id}/related",
                "input": {
                    "id": "string",
                    "depth": 1
                },
                "output": {
                    "relations": [{
                        "entry_id": "string",
                        "relation_type": "string",
                        "weight": 0.0
                    }]
                }
            }
        ],
        "links": {
            "docs": "https://github.com/youbanzhishi/OpenMind#readme",
            "source": "https://github.com/youbanzhishi/OpenMind",
            "health": "http://localhost:9090/api/v1/health"
        }
    }))
}
