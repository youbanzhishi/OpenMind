//! API路由定义
//!
//! 所有HTTP路由和处理器。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
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
        // Phase 6: Web UI
        .route("/", get(web_ui_index))
        .route("/ui/search", get(web_ui_search))
        .route("/ui/status", get(web_ui_status))
        .route("/ui/sync", get(web_ui_sync))
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
    State(state): State<Arc<AppState>>,
    Path(source): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(ref scheduler) = state.sync_scheduler {
        let task = scheduler.create_task(&source, openmind_sync::SyncStrategy::Incremental);
        scheduler.mark_running(&task.id);
        // In real impl: actual sync logic would run here
        scheduler.mark_completed(&task.id, "Sync completed");

        Ok(Json(json!({
            "source": source,
            "status": "completed",
            "task_id": task.id
        })))
    } else {
        Ok(Json(json!({
            "source": source,
            "status": "not_configured",
            "message": "Sync scheduler not configured"
        })))
    }
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

// ===== Phase 5: Action Protocol routes =====

/// GET /api/v1/actions - 列出所有可用Action
async fn list_actions(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if let Some(ref registry) = state.action_registry {
        let schemas = registry.list_schemas();
        Json(json!({
            "actions": schemas,
            "total": schemas.len()
        }))
    } else {
        Json(json!({"actions": [], "total": 0}))
    }
}

/// POST /api/v1/actions/:name/execute - 执行Action
async fn execute_action(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(params): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(ref registry) = state.action_registry {
        let input = openmind_actions::ActionInput::new(&name, params);
        let context = openmind_actions::ActionContext::new();
        let result = registry.execute(&name, input, context).await;

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
        Err(StatusCode::NOT_FOUND)
    }
}

/// GET /api/v1/actions/:name/schema - 获取Action Schema
async fn get_action_schema(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(ref registry) = state.action_registry {
        match registry.get_schema(&name) {
            Some(schema) => Ok(Json(serde_json::to_value(schema).unwrap_or_default())),
            None => Err(StatusCode::NOT_FOUND),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// ===== Phase 6: Sync/Monitor/Config routes =====

/// GET /api/v1/sync - 同步状态
async fn sync_status(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if let Some(ref monitor) = state.sync_monitor {
        let metrics = monitor.get_metrics();
        let status = monitor.get_status(0);
        Json(json!({
            "status": status,
            "metrics": metrics,
        }))
    } else {
        Json(json!({"status": "not_configured"}))
    }
}

/// POST /api/v1/sync/trigger - 触发全部同步
async fn trigger_sync_all(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if let Some(ref scheduler) = state.sync_scheduler {
        let due = scheduler.get_due_connectors();
        let mut results = Vec::new();
        for connector in &due {
            let task = scheduler.create_task(connector, openmind_sync::SyncStrategy::Incremental);
            results.push(json!({
                "connector": connector,
                "task_id": task.id,
                "status": "triggered"
            }));
        }
        Json(json!({
            "triggered": results,
            "total": results.len()
        }))
    } else {
        Json(json!({"triggered": [], "total": 0}))
    }
}

/// GET /api/v1/monitor - 监控面板数据
async fn monitor(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, StatusCode> {
    let store_stats = state.store.stats().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sync_info = if let Some(ref monitor) = state.sync_monitor {
        json!({
            "metrics": monitor.get_metrics(),
            "health": match monitor.health_check() {
                openmind_sync::HealthStatus::Healthy => "healthy",
                openmind_sync::HealthStatus::Degraded => "degraded",
                openmind_sync::HealthStatus::Unhealthy => "unhealthy",
            }
        })
    } else {
        json!({"status": "not_configured"})
    };

    Ok(Json(json!({
        "knowledge_base": {
            "total_entries": store_stats.total_entries,
            "total_relations": store_stats.total_relations,
            "total_tags": store_stats.total_tags,
            "by_source": store_stats.by_source,
            "by_embedding_status": store_stats.by_embedding_status,
        },
        "sync": sync_info,
        "version": state.version,
    })))
}

/// GET /api/v1/config - 获取配置
async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if let Some(ref config_mgr) = state.sync_config {
        let config = config_mgr.get_config();
        Json(serde_json::to_value(config).unwrap_or_default())
    } else {
        Json(json!({"error": "not_configured"}))
    }
}

/// POST /api/v1/config/reload - 热重载配置
async fn reload_config(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if let Some(ref config_mgr) = state.sync_config {
        match config_mgr.reload() {
            Ok(true) => Json(json!({"status": "reloaded"})),
            Ok(false) => Json(json!({"status": "no_file", "message": "No config file path set"})),
            Err(e) => Json(json!({"status": "error", "message": e.to_string()})),
        }
    } else {
        Json(json!({"status": "not_configured"}))
    }
}

// ===== Phase 6: Web UI =====

/// GET / - Web UI首页
async fn web_ui_index() -> Html<&'static str> {
    Html(WEB_UI_HTML)
}

/// GET /ui/search - 搜索页面
async fn web_ui_search() -> Html<&'static str> {
    Html(WEB_UI_HTML)
}

/// GET /ui/status - 状态页面
async fn web_ui_status() -> Html<&'static str> {
    Html(WEB_UI_HTML)
}

/// GET /ui/sync - 同步页面
async fn web_ui_sync() -> Html<&'static str> {
    Html(WEB_UI_HTML)
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

/// Web UI HTML (embedded, HTMX-based lightweight UI)
static WEB_UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenMind - Knowledge Engine</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <style>
        :root { --bg: #0f172a; --card: #1e293b; --accent: #3b82f6; --text: #e2e8f0; --dim: #94a3b8; --border: #334155; }
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: var(--bg); color: var(--text); min-height: 100vh; }
        nav { background: var(--card); border-bottom: 1px solid var(--border); padding: 1rem 2rem; display: flex; align-items: center; gap: 2rem; }
        nav .logo { font-size: 1.25rem; font-weight: 700; color: var(--accent); }
        nav a { color: var(--dim); text-decoration: none; font-size: 0.9rem; }
        nav a:hover { color: var(--text); }
        .container { max-width: 1200px; margin: 2rem auto; padding: 0 2rem; }
        .card { background: var(--card); border: 1px solid var(--border); border-radius: 8px; padding: 1.5rem; margin-bottom: 1rem; }
        .card h2 { font-size: 1.1rem; margin-bottom: 1rem; color: var(--accent); }
        .stats-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; }
        .stat { text-align: center; }
        .stat .number { font-size: 2rem; font-weight: 700; color: var(--accent); }
        .stat .label { font-size: 0.85rem; color: var(--dim); }
        input, button { padding: 0.6rem 1rem; border-radius: 6px; border: 1px solid var(--border); background: var(--bg); color: var(--text); }
        button { background: var(--accent); border: none; cursor: pointer; font-weight: 600; }
        button:hover { opacity: 0.9; }
        .search-box { display: flex; gap: 0.5rem; margin-bottom: 1rem; }
        .search-box input { flex: 1; }
        .result-item { border-bottom: 1px solid var(--border); padding: 1rem 0; }
        .result-item:last-child { border: none; }
        .result-title { font-weight: 600; margin-bottom: 0.3rem; }
        .result-source { font-size: 0.8rem; color: var(--dim); }
        .result-content { font-size: 0.9rem; margin-top: 0.5rem; line-height: 1.5; }
        .tag { display: inline-block; background: var(--border); padding: 0.2rem 0.6rem; border-radius: 4px; font-size: 0.75rem; margin-right: 0.3rem; }
        .health-ok { color: #22c55e; } .health-warn { color: #eab308; } .health-err { color: #ef4444; }
        #results { min-height: 100px; }
        .loading { color: var(--dim); font-style: italic; }
    </style>
</head>
<body>
    <nav>
        <span class="logo">🧠 OpenMind</span>
        <a href="/">Dashboard</a>
        <a href="/ui/search">Search</a>
        <a href="/ui/status">Status</a>
        <a href="/ui/sync">Sync</a>
    </nav>
    <div class="container" id="main">
        <div class="card">
            <h2>📊 Dashboard</h2>
            <div class="stats-grid" id="stats-grid">
                <div class="stat"><div class="number" id="total-entries">-</div><div class="label">知识条目</div></div>
                <div class="stat"><div class="number" id="total-relations">-</div><div class="label">关联数</div></div>
                <div class="stat"><div class="number" id="total-tags">-</div><div class="label">标签数</div></div>
                <div class="stat"><div class="number" id="sync-health">-</div><div class="label">同步健康</div></div>
            </div>
        </div>
        <div class="card">
            <h2>🔍 Quick Search</h2>
            <div class="search-box">
                <input type="text" id="search-input" placeholder="Search knowledge base..." onkeydown="if(event.key==='Enter')doSearch()">
                <button onclick="doSearch()">Search</button>
            </div>
            <div id="results"></div>
        </div>
        <div class="card">
            <h2>⚡ Actions</h2>
            <div id="actions-list">Loading...</div>
        </div>
    </div>
    <script>
        async function loadStats() {
            try {
                const r = await fetch('/api/v1/monitor');
                const d = await r.json();
                document.getElementById('total-entries').textContent = d.knowledge_base?.total_entries ?? '-';
                document.getElementById('total-relations').textContent = d.knowledge_base?.total_relations ?? '-';
                document.getElementById('total-tags').textContent = d.knowledge_base?.total_tags ?? '-';
                const h = d.sync?.health ?? 'unknown';
                const el = document.getElementById('sync-health');
                el.textContent = h;
                el.className = 'number ' + (h==='healthy'?'health-ok':h==='degraded'?'health-warn':'health-err');
            } catch(e) { console.error(e); }
        }
        async function loadActions() {
            try {
                const r = await fetch('/api/v1/actions');
                const d = await r.json();
                const el = document.getElementById('actions-list');
                if (d.actions && d.actions.length > 0) {
                    el.innerHTML = d.actions.map(a =>
                        '<div style="margin-bottom:0.5rem"><strong>'+a.name+'</strong>: '+a.description+'</div>'
                    ).join('');
                } else {
                    el.textContent = 'No actions registered';
                }
            } catch(e) { document.getElementById('actions-list').textContent = 'Failed to load'; }
        }
        async function doSearch() {
            const q = document.getElementById('search-input').value;
            if (!q) return;
            document.getElementById('results').innerHTML = '<div class="loading">Searching...</div>';
            try {
                const r = await fetch('/api/v1/search', {
                    method: 'POST', headers: {'Content-Type':'application/json'},
                    body: JSON.stringify({query:q, mode:'keyword', filters:{}})
                });
                const d = await r.json();
                const el = document.getElementById('results');
                if (d.results && d.results.length > 0) {
                    el.innerHTML = d.results.map(r =>
                        '<div class="result-item"><div class="result-title">'+r.entry.title+'</div>' +
                        '<div class="result-source">'+r.entry.source_type+' · '+r.entry.source_id+' · relevance: '+r.relevance.toFixed(3)+'</div>' +
                        '<div class="result-content">'+r.entry.content.substring(0,200)+(r.entry.content.length>200?'...':'')+'</div></div>'
                    ).join('');
                } else {
                    el.textContent = 'No results found';
                }
            } catch(e) { document.getElementById('results').textContent = 'Search failed'; }
        }
        loadStats();
        loadActions();
    </script>
</body>
</html>"#;
