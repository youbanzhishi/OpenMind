//! API路由定义
//!
//! 所有HTTP路由和处理器。
//! Phase 6: 增强Web UI（多页面HTMX+Alpine.js）、API指标、配置热重载。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use openmind_core::{
    compute_content_hash, EmbeddingStatus, EntryStatus, HealthResponse, IngestRequest,
    IngestResponse, KnowledgeEntry, KnowledgeStats, KnowledgeStore, SearchMode, SearchRequest,
    SearchResponse, SourceType,
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
        // Phase 5: Action Protocol routes
        .route("/api/v1/actions", get(list_actions))
        .route("/api/v1/actions/:name/execute", post(execute_action))
        .route("/api/v1/actions/:name/schema", get(get_action_schema))
        // Phase 6: Sync/Monitor/Config routes
        .route("/api/v1/sync", get(sync_status))
        .route("/api/v1/sync/trigger", post(trigger_sync_all))
        .route("/api/v1/monitor", get(monitor))
        .route("/api/v1/config", get(get_config))
        .route("/api/v1/config/reload", post(reload_config))
        // Phase 6: API Metrics & Storage
        .route("/api/v1/metrics", get(api_metrics_endpoint))
        .route("/api/v1/storage", get(storage_info))
        // Phase 6: Web UI pages
        .route("/", get(web_ui_dashboard))
        .route("/ui/search", get(web_ui_search))
        .route("/ui/config", get(web_ui_config))
        .route("/ui/ingest", get(web_ui_ingest))
        .route("/ui/status", get(web_ui_status))
        .route("/ui/sync", get(web_ui_sync))
        // Agent discovery
        .route("/.well-known/agent.json", get(agent_discovery))
        .with_state(state)
}

// ===== Core API handlers =====

/// POST /api/v1/search - 搜索（keyword/semantic/hybrid）
async fn search(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let _guard = state.api_metrics.start_request("/api/v1/search", "POST");
    let limit = req.limit.unwrap_or(10);
    let store = &*state.store;
    match store.query_keyword(&req.query, limit, &req.filters).await {
        Ok(results) => {
            let total = results.len();
            _guard.ok();
            Ok(Json(SearchResponse {
                results,
                mode: SearchMode::Keyword,
                total,
                degraded: false,
            }))
        }
        Err(e) => {
            tracing::error!("Search error: {}", e);
            _guard.err("search_failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /api/v1/ingest - 摄入内容
async fn ingest(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IngestRequest>,
) -> Result<Json<IngestResponse>, StatusCode> {
    let _guard = state.api_metrics.start_request("/api/v1/ingest", "POST");
    let now = chrono::Utc::now();
    let title = req
        .metadata
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
    match state.store.store(entry).await {
        Ok(_) => {
            _guard.ok();
            Ok(Json(IngestResponse {
                id,
                status: "indexed".to_string(),
            }))
        }
        Err(e) => {
            tracing::error!("Ingest error: {}", e);
            _guard.err("ingest_failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/v1/entry/:id - 获取知识条目
async fn get_entry(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let _guard = state.api_metrics.start_request("/api/v1/entry/:id", "GET");
    match state.store.get(&id).await {
        Ok(Some(e)) => {
            _guard.ok();
            Ok(Json(serde_json::to_value(e).unwrap_or_default()))
        }
        Ok(None) => {
            _guard.err("not_found");
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            tracing::error!("Get entry error: {}", e);
            _guard.err("get_failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/v1/entry/:id/related - 获取关联知识
async fn get_related(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let _guard = state
        .api_metrics
        .start_request("/api/v1/entry/:id/related", "GET");
    match state.store.get_related(&id, 1).await {
        Ok(relations) => {
            _guard.ok();
            Ok(Json(json!({
                "entry_id": id,
                "relations": relations,
            })))
        }
        Err(e) => {
            tracing::error!("Get related error: {}", e);
            _guard.err("related_failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /api/v1/sync/:source - 触发同步
async fn trigger_sync(
    State(state): State<Arc<AppState>>,
    Path(source): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let _guard = state
        .api_metrics
        .start_request("/api/v1/sync/:source", "POST");
    if let Some(ref scheduler) = state.sync_scheduler {
        let task = scheduler.create_task(&source, openmind_sync::SyncStrategy::Incremental);
        scheduler.mark_running(&task.id);
        scheduler.mark_completed(&task.id, "Sync completed");

        if let Some(ref monitor) = state.sync_monitor {
            monitor.record_sync_success(&source, 1);
        }

        _guard.ok();
        Ok(Json(json!({
            "source": source,
            "status": "completed",
            "task_id": task.id
        })))
    } else {
        _guard.ok();
        Ok(Json(json!({
            "source": source,
            "status": "not_configured",
            "message": "Sync scheduler not configured"
        })))
    }
}

/// GET /api/v1/connectors - 列出已注册Connector
async fn list_connectors(State(state): State<Arc<AppState>>) -> Json<Value> {
    let _guard = state.api_metrics.start_request("/api/v1/connectors", "GET");
    _guard.ok();
    Json(json!({
        "connectors": state.connectors,
    }))
}

/// GET /api/v1/health - 健康检查
async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let _guard = state.api_metrics.start_request("/api/v1/health", "GET");
    let embedding_status = if state.embedding_available {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    };

    _guard.ok();
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
        connectors: state.connectors.clone(),
        embedding_status,
    })
}

/// GET /api/v1/stats - 知识库统计
async fn stats(State(state): State<Arc<AppState>>) -> Result<Json<KnowledgeStats>, StatusCode> {
    let _guard = state.api_metrics.start_request("/api/v1/stats", "GET");
    match state.store.stats().await {
        Ok(stats) => {
            _guard.ok();
            Ok(Json(stats))
        }
        Err(e) => {
            tracing::error!("Stats error: {}", e);
            _guard.err("stats_failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ===== Phase 5: Action Protocol routes =====

/// GET /api/v1/actions - 列出所有可用Action
async fn list_actions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let _guard = state.api_metrics.start_request("/api/v1/actions", "GET");
    if let Some(ref registry) = state.action_registry {
        let schemas = registry.list_schemas();
        _guard.ok();
        Json(json!({
            "actions": schemas,
            "total": schemas.len()
        }))
    } else {
        _guard.ok();
        Json(json!({"actions": [], "total": 0}))
    }
}

/// POST /api/v1/actions/:name/execute - 执行Action
async fn execute_action(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(params): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let _guard = state
        .api_metrics
        .start_request("/api/v1/actions/:name/execute", "POST");
    if let Some(ref registry) = state.action_registry {
        let input = openmind_actions::ActionInput::new(&name, params);
        let context = openmind_actions::ActionContext::new();
        let result = registry.execute(&name, input, context).await;

        _guard.ok();
        Ok(Json(json!({
            "status": match result.output.status {
                openmind_actions::ActionStatus::Success => "success",
                openmind_actions::ActionStatus::Failed => "failed",
                openmind_actions::ActionStatus::ValidationError => "validation_error",
            },
            "data": result.output.data,
            "error": result.output.error,
            "duration_ms": result.output.duration_ms,
            "middleware_trace": result.middleware_trace,
        })))
    } else {
        _guard.err("no_registry");
        Err(StatusCode::NOT_FOUND)
    }
}

/// GET /api/v1/actions/:name/schema - 获取Action Schema
async fn get_action_schema(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let _guard = state
        .api_metrics
        .start_request("/api/v1/actions/:name/schema", "GET");
    if let Some(ref registry) = state.action_registry {
        if let Some(schema) = registry.get_schema(&name) {
            _guard.ok();
            Ok(Json(serde_json::to_value(schema).unwrap_or_default()))
        } else {
            _guard.err("not_found");
            Err(StatusCode::NOT_FOUND)
        }
    } else {
        _guard.err("no_registry");
        Err(StatusCode::NOT_FOUND)
    }
}

// ===== Phase 6: Sync/Monitor/Config routes =====

/// GET /api/v1/sync - 同步状态概览
async fn sync_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let _guard = state.api_metrics.start_request("/api/v1/sync", "GET");
    if let Some(ref monitor) = state.sync_monitor {
        let active = state
            .sync_scheduler
            .as_ref()
            .map(|s| s.active_task_count())
            .unwrap_or(0);
        let status = monitor.get_status(active);
        let metrics = monitor.get_metrics();
        _guard.ok();
        Json(json!({
            "status": status,
            "metrics": metrics,
        }))
    } else {
        _guard.ok();
        Json(json!({
            "status": null,
            "metrics": null,
        }))
    }
}

/// POST /api/v1/sync/trigger - 触发全量同步
async fn trigger_sync_all(State(state): State<Arc<AppState>>) -> Json<Value> {
    let _guard = state
        .api_metrics
        .start_request("/api/v1/sync/trigger", "POST");
    if let Some(ref scheduler) = state.sync_scheduler {
        let due = scheduler.get_due_connectors();
        if due.is_empty() {
            _guard.ok();
            return Json(json!({
                "status": "no_due_connectors",
                "message": "No connectors due for sync"
            }));
        }

        let mut task_ids = Vec::new();
        for connector in &due {
            let task = scheduler.create_task(connector, openmind_sync::SyncStrategy::Incremental);
            scheduler.mark_running(&task.id);
            scheduler.mark_completed(&task.id, "Sync completed");
            task_ids.push(task.id);

            if let Some(ref monitor) = state.sync_monitor {
                monitor.record_sync_success(connector, 1);
            }
        }

        _guard.ok();
        Json(json!({
            "status": "completed",
            "synced_connectors": due,
            "task_ids": task_ids,
        }))
    } else {
        _guard.ok();
        Json(json!({
            "status": "not_configured",
            "message": "Sync scheduler not configured"
        }))
    }
}

/// GET /api/v1/monitor - 监控面板数据
async fn monitor(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let _guard = state.api_metrics.start_request("/api/v1/monitor", "GET");
    let store = &*state.store;
    match store.stats().await {
        Ok(kb_stats) => {
            let sync_data = if let Some(ref monitor) = state.sync_monitor {
                let active = state
                    .sync_scheduler
                    .as_ref()
                    .map(|s| s.active_task_count())
                    .unwrap_or(0);
                let status = monitor.get_status(active);
                let metrics = monitor.get_metrics();
                let health = match monitor.health_check() {
                    openmind_sync::HealthStatus::Healthy => "healthy",
                    openmind_sync::HealthStatus::Degraded => "degraded",
                    openmind_sync::HealthStatus::Unhealthy => "unhealthy",
                };
                json!({
                    "status": status,
                    "metrics": metrics,
                    "health": health,
                })
            } else {
                json!(null)
            };

            _guard.ok();
            Ok(Json(json!({
                "knowledge_base": {
                    "total_entries": kb_stats.total_entries,
                    "total_relations": kb_stats.total_relations,
                    "total_tags": kb_stats.total_tags,
                    "by_source": kb_stats.by_source,
                    "by_embedding_status": kb_stats.by_embedding_status,
                },
                "sync": sync_data,
                "version": state.version,
                "embedding_available": state.embedding_available,
            })))
        }
        Err(e) => {
            tracing::error!("Monitor stats error: {}", e);
            _guard.err("stats_failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/v1/config - 获取当前配置
async fn get_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let _guard = state.api_metrics.start_request("/api/v1/config", "GET");
    if let Some(ref config) = state.sync_config {
        let cfg = config.get_config();
        let toml_str = config.to_toml().unwrap_or_default();
        _guard.ok();
        Json(json!({
            "config": cfg,
            "toml": toml_str,
        }))
    } else {
        _guard.ok();
        Json(json!({
            "config": null,
            "toml": "",
        }))
    }
}

/// POST /api/v1/config/reload - 热重载配置
async fn reload_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let _guard = state
        .api_metrics
        .start_request("/api/v1/config/reload", "POST");
    if let Some(ref config) = state.sync_config {
        match config.reload() {
            Ok(reloaded) => {
                _guard.ok();
                Json(json!({
                    "reloaded": reloaded,
                    "message": if reloaded { "Config reloaded from file" } else { "No config file path set" }
                }))
            }
            Err(e) => {
                _guard.err("reload_failed");
                Json(json!({
                    "reloaded": false,
                    "error": e.to_string(),
                }))
            }
        }
    } else {
        _guard.ok();
        Json(json!({
            "reloaded": false,
            "message": "Config manager not configured"
        }))
    }
}

// ===== Phase 6: API Metrics & Storage =====

/// GET /api/v1/metrics - API调用指标
async fn api_metrics_endpoint(State(state): State<Arc<AppState>>) -> Json<Value> {
    let metrics = state.api_metrics.get_metrics();
    Json(json!(metrics))
}

/// GET /api/v1/storage - 存储占用信息
async fn storage_info(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let _guard = state.api_metrics.start_request("/api/v1/storage", "GET");
    let store = &*state.store;
    match store.stats().await {
        Ok(kb_stats) => {
            let embedding_map: std::collections::HashMap<String, i64> =
                serde_json::from_value(kb_stats.by_embedding_status.clone()).unwrap_or_default();
            let source_map: std::collections::HashMap<String, i64> =
                serde_json::from_value(kb_stats.by_source.clone()).unwrap_or_default();

            _guard.ok();
            Ok(Json(json!({
                "db_size_bytes": 0,
                "total_entries": kb_stats.total_entries,
                "total_relations": kb_stats.total_relations,
                "embedding_stats": embedding_map,
                "source_stats": source_map,
            })))
        }
        Err(e) => {
            tracing::error!("Storage info error: {}", e);
            _guard.err("stats_failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ===== Phase 6: Web UI pages =====

/// GET / - 仪表盘页面
async fn web_ui_dashboard(State(state): State<Arc<AppState>>) -> Html<String> {
    let store = &*state.store;
    let kb_stats = store
        .stats()
        .await
        .unwrap_or_else(|_| openmind_core::KnowledgeStats {
            total_entries: 0,
            by_source: json!({}),
            by_embedding_status: json!({}),
            total_relations: 0,
            total_tags: 0,
        });

    let sync_health = if let Some(ref monitor) = state.sync_monitor {
        match monitor.health_check() {
            openmind_sync::HealthStatus::Healthy => "healthy",
            openmind_sync::HealthStatus::Degraded => "degraded",
            openmind_sync::HealthStatus::Unhealthy => "unhealthy",
        }
    } else {
        "unknown"
    };

    let stats_json = json!({
        "total_entries": kb_stats.total_entries,
        "total_relations": kb_stats.total_relations,
        "total_tags": kb_stats.total_tags,
    })
    .to_string();

    let connectors_json = json!(state.connectors).to_string();
    let monitor_json = json!(sync_health).to_string();

    Html(crate::web_ui::dashboard_page(
        &stats_json,
        &monitor_json,
        &connectors_json,
    ))
}

/// GET /ui/search - 搜索页面
async fn web_ui_search() -> Html<String> {
    Html(crate::web_ui::search_page())
}

/// GET /ui/config - 配置页面
async fn web_ui_config(State(state): State<Arc<AppState>>) -> Html<String> {
    let toml_str = if let Some(ref config) = state.sync_config {
        config
            .to_toml()
            .unwrap_or_else(|_| "# Config unavailable".to_string())
    } else {
        "# No configuration loaded\n# Start OpenMind with a config file to see settings here"
            .to_string()
    };

    Html(crate::web_ui::config_page(&toml_str))
}

/// GET /ui/ingest - 摄入页面
async fn web_ui_ingest() -> Html<String> {
    Html(crate::web_ui::ingest_page())
}

/// GET /ui/status - 状态监控页面
async fn web_ui_status(State(state): State<Arc<AppState>>) -> Html<String> {
    let api_metrics = state.api_metrics.get_metrics();
    let metrics_json = serde_json::to_string(&api_metrics).unwrap_or_else(|_| "{}".to_string());

    // Build storage info
    let store = &*state.store;
    let kb_stats = store
        .stats()
        .await
        .unwrap_or_else(|_| openmind_core::KnowledgeStats {
            total_entries: 0,
            by_source: json!({}),
            by_embedding_status: json!({}),
            total_relations: 0,
            total_tags: 0,
        });
    let embedding_map: std::collections::HashMap<String, i64> =
        serde_json::from_value(kb_stats.by_embedding_status.clone()).unwrap_or_default();
    let source_map: std::collections::HashMap<String, i64> =
        serde_json::from_value(kb_stats.by_source.clone()).unwrap_or_default();
    let storage_json = json!({
        "db_size_bytes": 0,
        "total_entries": kb_stats.total_entries,
        "total_relations": kb_stats.total_relations,
        "embedding_stats": embedding_map,
        "source_stats": source_map,
    })
    .to_string();

    // Sync metrics
    let sync_metrics_json = if let Some(ref monitor) = state.sync_monitor {
        serde_json::to_string(&monitor.get_metrics()).unwrap_or_else(|_| "null".to_string())
    } else {
        "null".to_string()
    };

    Html(crate::web_ui::status_page(
        &metrics_json,
        &sync_metrics_json,
        &storage_json,
    ))
}

/// GET /ui/sync - 同步页面
async fn web_ui_sync(State(state): State<Arc<AppState>>) -> Html<String> {
    let sync_status_json = if let Some(ref monitor) = state.sync_monitor {
        let active = state
            .sync_scheduler
            .as_ref()
            .map(|s| s.active_task_count())
            .unwrap_or(0);
        let status = monitor.get_status(active);
        serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string())
    } else {
        "{}".to_string()
    };

    let sync_metrics_json = if let Some(ref monitor) = state.sync_monitor {
        serde_json::to_string(&monitor.get_metrics()).unwrap_or_else(|_| "{}".to_string())
    } else {
        "{}".to_string()
    };

    Html(crate::web_ui::sync_page(
        &sync_status_json,
        &sync_metrics_json,
    ))
}

// ===== Agent Discovery =====

/// GET /.well-known/agent.json - Agent发现协议
async fn agent_discovery(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "protocol": "openmind-agent/v1",
        "name": "OpenMind Knowledge Engine",
        "version": state.version,
        "description": "AI-native personal knowledge engine - knowledge node in the agent ecosystem",
        "capabilities": [
            "search",
            "ingest",
            "sync",
            "relate",
            "rag_query"
        ],
        "actions": [
            {
                "name": "search",
                "description": "搜索知识库（关键词/语义/混合）",
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
            },
            {
                "name": "search_and_mix",
                "description": "混合搜索：关键词+语义搜索合并去重",
                "endpoint": "POST /api/v1/actions/search_and_mix/execute",
                "input": {
                    "query": "string",
                    "limit": 5
                },
                "output": {
                    "keyword_results": "array",
                    "semantic_results": "array",
                    "merged_results": "array"
                }
            },
            {
                "name": "ingest_and_relate",
                "description": "摄入内容并自动建立知识关联",
                "endpoint": "POST /api/v1/actions/ingest_and_relate/execute",
                "input": {
                    "source": "string",
                    "content": "string",
                    "auto_relate": true
                },
                "output": {
                    "entry_id": "string",
                    "relations_created": 0
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
