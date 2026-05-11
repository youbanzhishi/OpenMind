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
    compute_content_hash, EmbeddingStatus, EntryStatus,
    HealthResponse, IngestRequest, IngestResponse, KnowledgeEntry,
    KnowledgeStats, KnowledgeStore, SearchMode,
    SearchRequest, SearchResponse, SourceType,
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

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
        .route("/api/v1/stats", get(stats))
        // Agent discovery
        .route("/.well-known/agent.json", get(agent_discovery))
        .with_state(state)
}

/// POST /api/v1/search - 搜索（keyword/semantic/hybrid）
async fn search(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let limit = req.limit.unwrap_or(10);
    let store = &*state.store;
    let results = store
        .query_keyword(&req.query, limit, &req.filters)
        .await
        .map_err(|e| {
            tracing::error!("Search error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total = results.len();
    Ok(Json(SearchResponse {
        results,
        mode: SearchMode::Keyword,
        total,
        degraded: false,
    }))
}

/// POST /api/v1/ingest - 摄入内容
async fn ingest(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IngestRequest>,
) -> Result<Json<IngestResponse>, StatusCode> {
    let now = chrono::Utc::now();
    let title = req.metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let entry = KnowledgeEntry {
        id: Uuid::new_v4().to_string(),
        source_type: SourceType::File,
        source_id: req.source.clone(),
        title: title.to_string(),
        content: req.content.clone(),
        content_hash: compute_content_hash(&req.content),
        embedding_id: None,
        embedding_status: EmbeddingStatus::Pending,
        tags: req.tags.clone(),
        project: None,
        metadata: req.metadata.clone(),
        file_references: vec![],
        created_at: now,
        updated_at: now,
        status: EntryStatus::Active,
    };

    let id = entry.id.clone();
    state.store.store(entry).await.map_err(|e| {
        tracing::error!("Ingest error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(IngestResponse {
        id,
        status: "indexed".to_string(),
    }))
}

/// GET /api/v1/entry/:id - 获取知识条目
async fn get_entry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let entry = state.store.get(&id).await.map_err(|e| {
        tracing::error!("Get entry error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match entry {
        Some(e) => Ok(Json(serde_json::to_value(e).unwrap_or_default())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /api/v1/entry/:id/related - 获取关联知识
async fn get_related(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let relations = state.store.get_related(&id, 1).await.map_err(|e| {
        tracing::error!("Get related error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "entry_id": id,
        "relations": relations,
    })))
}

/// POST /api/v1/sync/:source - 触发同步
async fn trigger_sync(
    State(_state): State<Arc<AppState>>,
    Path(source): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // TODO: Implement with actual connector
    Ok(Json(json!({
        "source": source,
        "status": "not_implemented",
        "message": "Connector sync not yet implemented"
    })))
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
    let embedding_status = if state.embedding_available {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
        connectors: state.connectors.clone(),
        embedding_status,
    })
}

/// GET /api/v1/stats - 知识库统计
async fn stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<KnowledgeStats>, StatusCode> {
    let stats = state.store.stats().await.map_err(|e| {
        tracing::error!("Stats error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(stats))
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
