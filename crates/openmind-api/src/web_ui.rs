//! Web UI 模板
//!
//! 内嵌HTML模板，使用HTMX+Alpine.js实现轻量交互。
//! 服务端渲染片段，前端只做展示和交互，逻辑全在API层。

/// 共享导航栏
pub fn nav_html(active: &str) -> String {
    let items = [
        ("dashboard", "/", "📊 Dashboard"),
        ("search", "/ui/search", "🔍 Search"),
        ("config", "/ui/config", "⚙️ Config"),
        ("ingest", "/ui/ingest", "📥 Ingest"),
        ("status", "/ui/status", "🩺 Status"),
        ("sync", "/ui/sync", "🔄 Sync"),
    ];
    let links: Vec<String> = items
        .iter()
        .map(|(key, href, label)| {
            if *key == active {
                format!(
                    "<a href=\"{}\" style=\"color:var(--accent);font-weight:600\">{}</a>",
                    href, label
                )
            } else {
                format!("<a href=\"{}\">{}</a>", href, label)
            }
        })
        .collect();
    format!(
        r#"<nav>
        <span class="logo">🧠 OpenMind</span>
        {}
    </nav>"#,
        links.join("\n        ")
    )
}

/// 共享CSS样式
pub fn style_css() -> &'static str {
    r#"
        :root { --bg: #0f172a; --card: #1e293b; --accent: #3b82f6; --accent2: #8b5cf6; --text: #e2e8f0; --dim: #94a3b8; --border: #334155; --ok: #22c55e; --warn: #eab308; --err: #ef4444; }
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: var(--bg); color: var(--text); min-height: 100vh; }
        nav { background: var(--card); border-bottom: 1px solid var(--border); padding: 1rem 2rem; display: flex; align-items: center; gap: 2rem; position: sticky; top: 0; z-index: 10; }
        nav .logo { font-size: 1.25rem; font-weight: 700; color: var(--accent); }
        nav a { color: var(--dim); text-decoration: none; font-size: 0.9rem; transition: color 0.2s; }
        nav a:hover { color: var(--text); }
        .container { max-width: 1200px; margin: 2rem auto; padding: 0 2rem; }
        .card { background: var(--card); border: 1px solid var(--border); border-radius: 8px; padding: 1.5rem; margin-bottom: 1rem; }
        .card h2 { font-size: 1.1rem; margin-bottom: 1rem; color: var(--accent); }
        .stats-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 1rem; }
        .stat { text-align: center; padding: 1rem; }
        .stat .number { font-size: 2rem; font-weight: 700; color: var(--accent); }
        .stat .label { font-size: 0.85rem; color: var(--dim); margin-top: 0.3rem; }
        input, select, textarea { padding: 0.6rem 1rem; border-radius: 6px; border: 1px solid var(--border); background: var(--bg); color: var(--text); font-size: 0.9rem; }
        input:focus, select:focus, textarea:focus { outline: none; border-color: var(--accent); }
        button, .btn { padding: 0.6rem 1.2rem; border-radius: 6px; border: none; cursor: pointer; font-weight: 600; font-size: 0.9rem; background: var(--accent); color: white; transition: opacity 0.2s; }
        button:hover, .btn:hover { opacity: 0.85; }
        button.secondary { background: var(--border); }
        button.danger { background: var(--err); }
        .search-box { display: flex; gap: 0.5rem; margin-bottom: 1rem; }
        .search-box input { flex: 1; }
        .result-item { border-bottom: 1px solid var(--border); padding: 1rem 0; }
        .result-item:last-child { border: none; }
        .result-title { font-weight: 600; margin-bottom: 0.3rem; }
        .result-source { font-size: 0.8rem; color: var(--dim); }
        .result-content { font-size: 0.9rem; margin-top: 0.5rem; line-height: 1.5; }
        .tag { display: inline-block; background: var(--border); padding: 0.2rem 0.6rem; border-radius: 4px; font-size: 0.75rem; margin-right: 0.3rem; }
        .health-ok { color: var(--ok); } .health-warn { color: var(--warn); } .health-err { color: var(--err); }
        .loading { color: var(--dim); font-style: italic; }
        table { width: 100%; border-collapse: collapse; }
        th, td { text-align: left; padding: 0.6rem 1rem; border-bottom: 1px solid var(--border); font-size: 0.85rem; }
        th { color: var(--dim); font-weight: 600; }
        .toggle { position: relative; width: 44px; height: 24px; background: var(--border); border-radius: 12px; cursor: pointer; transition: background 0.3s; }
        .toggle.active { background: var(--accent); }
        .toggle::after { content: ''; position: absolute; top: 2px; left: 2px; width: 20px; height: 20px; background: white; border-radius: 50%; transition: transform 0.3s; }
        .toggle.active::after { transform: translateX(20px); }
        .form-group { margin-bottom: 1rem; }
        .form-group label { display: block; font-size: 0.85rem; color: var(--dim); margin-bottom: 0.3rem; }
        .form-group input, .form-group select, .form-group textarea { width: 100%; }
        .progress-bar { height: 6px; background: var(--border); border-radius: 3px; overflow: hidden; }
        .progress-bar .fill { height: 100%; background: var(--accent); border-radius: 3px; transition: width 0.5s; }
        .badge { display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px; font-size: 0.7rem; font-weight: 600; }
        .badge-ok { background: rgba(34,197,94,0.2); color: var(--ok); }
        .badge-warn { background: rgba(234,179,8,0.2); color: var(--warn); }
        .badge-err { background: rgba(239,68,68,0.2); color: var(--err); }
        .badge-info { background: rgba(59,130,246,0.2); color: var(--accent); }
        .two-col { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
        @media (max-width: 768px) { .two-col { grid-template-columns: 1fr; } .stats-grid { grid-template-columns: repeat(2, 1fr); } }
        pre { background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 1rem; overflow-x: auto; font-size: 0.8rem; line-height: 1.5; }
    "#
}

/// 仪表盘页面
pub fn dashboard_page(stats_json: &str, monitor_json: &str, connectors_json: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenMind - Dashboard</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
    <style>{style}</style>
</head>
<body>
    {nav}
    <div class="container" x-data="dashboard()">
        <!-- Stats Cards -->
        <div class="stats-grid" style="margin-bottom:1rem">
            <div class="card stat">
                <div class="number" x-text="stats.total_entries">-</div>
                <div class="label">知识条目</div>
            </div>
            <div class="card stat">
                <div class="number" x-text="stats.total_relations">-</div>
                <div class="label">关联数</div>
            </div>
            <div class="card stat">
                <div class="number" x-text="stats.total_tags">-</div>
                <div class="label">标签数</div>
            </div>
            <div class="card stat">
                <div class="number" :class="syncHealthClass" x-text="syncHealth">-</div>
                <div class="label">同步健康</div>
            </div>
        </div>

        <div class="two-col">
            <!-- Connectors -->
            <div class="card">
                <h2>🔌 Connectors</h2>
                <div id="connectors-list">
                    <template x-for="c in connectors" :key="c">
                        <div style="display:flex;justify-content:space-between;align-items:center;padding:0.5rem 0;border-bottom:1px solid var(--border)">
                            <span x-text="c"></span>
                            <span class="badge badge-ok">registered</span>
                        </div>
                    </template>
                    <div x-show="connectors.length===0" class="loading">No connectors registered</div>
                </div>
            </div>

            <!-- Quick Search -->
            <div class="card">
                <h2>🔍 Quick Search</h2>
                <div class="search-box">
                    <input type="text" x-model="searchQuery" @keydown.enter="doSearch()" placeholder="Search knowledge base...">
                    <button @click="doSearch()">Search</button>
                </div>
                <div id="quick-results">
                    <div x-show="searchLoading" class="loading">Searching...</div>
                    <template x-for="r in searchResults" :key="r.entry.id">
                        <div class="result-item">
                            <div class="result-title" x-text="r.entry.title"></div>
                            <div class="result-source" x-text="r.entry.source_type + ' · ' + r.entry.source_id + ' · relevance: ' + r.relevance.toFixed(3)"></div>
                            <div class="result-content" x-text="r.entry.content.substring(0,150) + (r.entry.content.length>150?'...':'')"></div>
                        </div>
                    </template>
                    <div x-show="searchResults.length===0 && !searchLoading && searchDone" class="loading">No results found</div>
                </div>
            </div>
        </div>

        <!-- Recent Sync Tasks -->
        <div class="card">
            <h2>📋 Recent Sync Activity</h2>
            <div hx-get="/api/v1/sync" hx-trigger="load, every 30s" hx-swap="innerHTML">
                Loading sync status...
            </div>
        </div>
    </div>
    <script>
        function dashboard() {{
            return {{
                stats: {stats},
                connectors: {connectors},
                syncHealth: {sync_health_val},
                syncHealthClass: '',
                searchQuery: '',
                searchResults: [],
                searchLoading: false,
                searchDone: false,
                init() {{
                    this.updateSyncHealthClass();
                }},
                updateSyncHealthClass() {{
                    this.syncHealthClass = this.syncHealth==='healthy'?'health-ok':this.syncHealth==='degraded'?'health-warn':'health-err';
                }},
                async doSearch() {{
                    if (!this.searchQuery) return;
                    this.searchLoading = true;
                    this.searchDone = false;
                    try {{
                        const r = await fetch('/api/v1/search', {{
                            method: 'POST',
                            headers: {{'Content-Type':'application/json'}},
                            body: JSON.stringify({{query:this.searchQuery, mode:'keyword', filters:{{}}}})
                        }});
                        const d = await r.json();
                        this.searchResults = d.results || [];
                    }} catch(e) {{ this.searchResults = []; }}
                    this.searchLoading = false;
                    this.searchDone = true;
                }}
            }}
        }}
    </script>
</body>
</html>"##,
        style = style_css(),
        nav = nav_html("dashboard"),
        stats = stats_json,
        connectors = connectors_json,
        sync_health_val = monitor_json,
    )
}

/// 搜索页面
pub fn search_page() -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenMind - Search</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
    <style>{style}</style>
</head>
<body>
    {nav}
    <div class="container" x-data="searchPage()">
        <div class="card">
            <h2>🔍 Knowledge Search</h2>
            <div class="search-box">
                <input type="text" x-model="query" @keydown.enter="doSearch()" placeholder="Enter search query...">
                <select x-model="mode" style="width:140px">
                    <option value="keyword">Keyword</option>
                    <option value="semantic">Semantic</option>
                    <option value="hybrid">Hybrid</option>
                </select>
                <button @click="doSearch()">Search</button>
            </div>
            <div style="display:flex;gap:1rem;margin-bottom:1rem;align-items:center">
                <div class="form-group" style="margin:0;flex:1">
                    <label>Source Filter</label>
                    <select x-model="filterSource" style="width:100%">
                        <option value="">All Sources</option>
                        <option value="blog">Blog</option>
                        <option value="vault">Vault</option>
                        <option value="bookmark">Bookmark</option>
                        <option value="note">Note</option>
                        <option value="file">File</option>
                    </select>
                </div>
                <div class="form-group" style="margin:0;flex:1">
                    <label>Tags (comma separated)</label>
                    <input type="text" x-model="filterTags" placeholder="tag1, tag2">
                </div>
                <div class="form-group" style="margin:0;width:80px">
                    <label>Limit</label>
                    <input type="number" x-model.number="limit" min="1" max="100">
                </div>
            </div>
        </div>

        <div class="card">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem">
                <h2 style="margin:0">Results <span x-show="total>0" class="badge badge-info" x-text="total + ' found'"></span></h2>
                <span x-show="degraded" class="badge badge-warn">Degraded Mode</span>
            </div>
            <div x-show="loading" class="loading">Searching...</div>
            <template x-for="r in results" :key="r.entry.id">
                <div class="result-item">
                    <div class="result-title" x-text="r.entry.title || '(untitled)'"></div>
                    <div class="result-source">
                        <span class="badge badge-info" x-text="r.entry.source_type"></span>
                        <span x-text="r.entry.source_id" style="margin-left:0.5rem"></span>
                        <span style="margin-left:0.5rem;color:var(--accent)" x-text="'relevance: ' + r.relevance.toFixed(4)"></span>
                    </div>
                    <template x-for="t in r.entry.tags" :key="t">
                        <span class="tag" x-text="t"></span>
                    </template>
                    <div class="result-content" x-text="r.entry.content.substring(0,300) + (r.entry.content.length>300?'...':'')"></div>
                    <template x-for="h in r.highlights" :key="h">
                        <div style="margin-top:0.3rem;padding:0.3rem 0.5rem;background:rgba(59,130,246,0.1);border-radius:4px;font-size:0.8rem" x-text="h"></div>
                    </template>
                </div>
            </template>
            <div x-show="results.length===0 && !loading && done" class="loading">No results found. Try a different query.</div>
        </div>
    </div>
    <script>
        function searchPage() {{
            return {{
                query: '', mode: 'keyword', filterSource: '', filterTags: '',
                limit: 10, results: [], total: 0, degraded: false,
                loading: false, done: false,
                async doSearch() {{
                    if (!this.query) return;
                    this.loading = true; this.done = false;
                    const tags = this.filterTags ? this.filterTags.split(',').map(t=>t.trim()).filter(Boolean) : [];
                    const filters = {{}};
                    if (this.filterSource) filters.source = this.filterSource;
                    if (tags.length) filters.tags = tags;
                    try {{
                        const r = await fetch('/api/v1/search', {{
                            method:'POST',
                            headers:{{'Content-Type':'application/json'}},
                            body: JSON.stringify({{
                                query: this.query,
                                mode: this.mode,
                                limit: this.limit,
                                filters: filters
                            }})
                        }});
                        const d = await r.json();
                        this.results = d.results || [];
                        this.total = d.total || 0;
                        this.degraded = d.degraded || false;
                    }} catch(e) {{ this.results = []; this.total = 0; }}
                    this.loading = false; this.done = true;
                }}
            }}
        }}
    </script>
</body>
</html>"##,
        style = style_css(),
        nav = nav_html("search"),
    )
}

/// 配置页面
pub fn config_page(config_toml: &str) -> String {
    // Escape the TOML for safe embedding in JS template literal
    let escaped_toml = config_toml
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");
    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenMind - Configuration</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
    <style>{style}</style>
</head>
<body>
    {nav}
    <div class="container" x-data="configPage()">
        <div class="two-col">
            <!-- Feature Toggles -->
            <div class="card">
                <h2>🎛️ Feature Toggles</h2>
                <div style="display:flex;justify-content:space-between;align-items:center;padding:0.8rem 0;border-bottom:1px solid var(--border)">
                    <div>
                        <div style="font-weight:600">Scheduled Sync</div>
                        <div style="font-size:0.8rem;color:var(--dim)">Enable periodic sync scheduling</div>
                    </div>
                    <div class="toggle" :class="{{'active': schedulerEnabled}}" @click="schedulerEnabled=!schedulerEnabled"></div>
                </div>
                <div style="display:flex;justify-content:space-between;align-items:center;padding:0.8rem 0;border-bottom:1px solid var(--border)">
                    <div>
                        <div style="font-weight:600">Incremental Sync</div>
                        <div style="font-size:0.8rem;color:var(--dim)">Only sync changed content</div>
                    </div>
                    <div class="toggle" :class="{{'active': incrementalSync}}" @click="incrementalSync=!incrementalSync"></div>
                </div>
                <div style="display:flex;justify-content:space-between;align-items:center;padding:0.8rem 0;border-bottom:1px solid var(--border)">
                    <div>
                        <div style="font-weight:600">Embedding Pipeline</div>
                        <div style="font-size:0.8rem;color:var(--dim)">Generate embeddings on ingest</div>
                    </div>
                    <div class="toggle" :class="{{'active': embeddingEnabled}}" @click="embeddingEnabled=!embeddingEnabled"></div>
                </div>
                <div style="display:flex;justify-content:space-between;align-items:center;padding:0.8rem 0">
                    <div>
                        <div style="font-weight:600">Conflict Auto-Resolve</div>
                        <div style="font-size:0.8rem;color:var(--dim)">Automatically resolve sync conflicts</div>
                    </div>
                    <div class="toggle" :class="{{'active': autoResolve}}" @click="autoResolve=!autoResolve"></div>
                </div>
            </div>

            <!-- Sync Settings -->
            <div class="card">
                <h2>⏱️ Sync Settings</h2>
                <div class="form-group">
                    <label>Default Sync Interval (seconds)</label>
                    <input type="number" x-model.number="syncInterval" min="30">
                </div>
                <div class="form-group">
                    <label>Max Concurrent Syncs</label>
                    <input type="number" x-model.number="maxConcurrent" min="1" max="10">
                </div>
                <div class="form-group">
                    <label>Batch Size</label>
                    <input type="number" x-model.number="batchSize" min="10" max="1000">
                </div>
                <div class="form-group">
                    <label>Conflict Strategy</label>
                    <select x-model="conflictStrategy">
                        <option value="last_write_wins">Last Write Wins</option>
                        <option value="source_priority">Source Priority</option>
                        <option value="manual">Manual Resolution</option>
                        <option value="merge">Merge</option>
                    </select>
                </div>
                <div class="form-group">
                    <label>Delete Mode</label>
                    <select x-model="deleteMode">
                        <option value="cascade">Cascade</option>
                        <option value="soft">Soft Delete</option>
                        <option value="ignore">Ignore</option>
                    </select>
                </div>
            </div>
        </div>

        <!-- TOML Config View -->
        <div class="card">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem">
                <h2 style="margin:0">📝 Current Configuration (TOML)</h2>
                <div style="display:flex;gap:0.5rem">
                    <button class="secondary" @click="reloadConfig()">🔄 Reload from File</button>
                    <button @click="applyConfig()">✅ Apply Changes</button>
                </div>
            </div>
            <pre x-text="configToml" style="max-height:400px;overflow-y:auto"></pre>
            <div x-show="message" style="margin-top:0.5rem">
                <span :class="messageType==='success'?'badge badge-ok':messageType==='error'?'badge badge-err':'badge badge-info'" x-text="message"></span>
            </div>
        </div>
    </div>
    <script>
        function configPage() {{
            return {{
                schedulerEnabled: true,
                incrementalSync: true,
                embeddingEnabled: true,
                autoResolve: true,
                syncInterval: 300,
                maxConcurrent: 3,
                batchSize: 100,
                conflictStrategy: 'last_write_wins',
                deleteMode: 'cascade',
                configToml: `{toml}`,
                message: '',
                messageType: 'info',
                async reloadConfig() {{
                    this.message = 'Reloading...'; this.messageType = 'info';
                    try {{
                        const r = await fetch('/api/v1/config/reload', {{method:'POST'}});
                        const d = await r.json();
                        if (d.reloaded) {{
                            this.message = 'Config reloaded'; this.messageType = 'success';
                            // Refresh TOML view
                            const cr = await fetch('/api/v1/config');
                            const cd = await cr.json();
                            if (cd.toml) this.configToml = cd.toml;
                        }} else {{
                            this.message = d.message || 'No file to reload'; this.messageType = 'warn';
                        }}
                    }} catch(e) {{ this.message = 'Reload failed'; this.messageType = 'error'; }}
                }},
                async applyConfig() {{
                    this.message = 'Applying...'; this.messageType = 'info';
                    // In a full impl, this would POST the updated config
                    this.message = 'Configuration applied (simulated)'; this.messageType = 'success';
                }}
            }}
        }}
    </script>
</body>
</html>"##,
        style = style_css(),
        nav = nav_html("config"),
        toml = escaped_toml,
    )
}

/// 摄入页面
pub fn ingest_page() -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenMind - Ingest</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
    <style>{style}</style>
</head>
<body>
    {nav}
    <div class="container" x-data="ingestPage()">
        <div class="two-col">
            <!-- Manual Ingest Form -->
            <div class="card">
                <h2>📥 Manual Ingest</h2>
                <div class="form-group">
                    <label>Source *</label>
                    <input type="text" x-model="source" placeholder="e.g., manual/notes">
                </div>
                <div class="form-group">
                    <label>Title</label>
                    <input type="text" x-model="title" placeholder="Content title">
                </div>
                <div class="form-group">
                    <label>Content *</label>
                    <textarea x-model="content" rows="8" placeholder="Paste or type content to ingest..."></textarea>
                </div>
                <div class="form-group">
                    <label>Tags (comma separated)</label>
                    <input type="text" x-model="tags" placeholder="tag1, tag2, tag3">
                </div>
                <div style="display:flex;gap:0.5rem">
                    <button @click="doIngest()" :disabled="ingesting">
                        <span x-show="!ingesting">🚀 Ingest</span>
                        <span x-show="ingesting">⏳ Ingesting...</span>
                    </button>
                    <button class="secondary" @click="clearForm()">Clear</button>
                </div>
                <div x-show="result" style="margin-top:1rem">
                    <span class="badge badge-ok" x-show="resultStatus==='success'" x-text="'Created: ' + result"></span>
                    <span class="badge badge-err" x-show="resultStatus==='error'" x-text="'Error: ' + result"></span>
                </div>
            </div>

            <!-- Trigger Sync -->
            <div>
                <div class="card">
                    <h2>🔄 Trigger Sync</h2>
                    <p style="color:var(--dim);font-size:0.85rem;margin-bottom:1rem">
                        Trigger synchronization for a specific connector or all connectors.
                    </p>
                    <div class="form-group">
                        <label>Connector</label>
                        <select x-model="syncSource">
                            <option value="">All Connectors</option>
                            <option value="vault">Vault</option>
                            <option value="blog">Blog</option>
                            <option value="bookmark">Bookmark</option>
                            <option value="note">Note</option>
                        </select>
                    </div>
                    <div class="form-group">
                        <label>Sync Strategy</label>
                        <select x-model="syncStrategy">
                            <option value="incremental">Incremental</option>
                            <option value="full">Full</option>
                        </select>
                    </div>
                    <button @click="triggerSync()" :disabled="syncing">
                        <span x-show="!syncing">⚡ Trigger Sync</span>
                        <span x-show="syncing">⏳ Syncing...</span>
                    </button>
                    <div x-show="syncResult" style="margin-top:1rem">
                        <span class="badge badge-ok" x-show="syncResultStatus==='success'" x-text="syncResult"></span>
                        <span class="badge badge-err" x-show="syncResultStatus==='error'" x-text="syncResult"></span>
                    </div>
                </div>

                <div class="card">
                    <h2>📊 Ingest History</h2>
                    <div style="color:var(--dim);font-size:0.85rem">
                        Recent ingest operations will appear here.
                    </div>
                    <template x-for="h in history" :key="h.id">
                        <div style="padding:0.5rem 0;border-bottom:1px solid var(--border)">
                            <div style="display:flex;justify-content:space-between">
                                <span style="font-weight:600" x-text="h.source"></span>
                                <span class="badge badge-ok" x-text="h.status"></span>
                            </div>
                            <div style="font-size:0.8rem;color:var(--dim)" x-text="h.id"></div>
                        </div>
                    </template>
                </div>
            </div>
        </div>
    </div>
    <script>
        function ingestPage() {{
            return {{
                source: '', title: '', content: '', tags: '',
                ingesting: false, result: '', resultStatus: '',
                syncSource: '', syncStrategy: 'incremental',
                syncing: false, syncResult: '', syncResultStatus: '',
                history: [],
                async doIngest() {{
                    if (!this.source || !this.content) {{
                        this.result = 'Source and content are required';
                        this.resultStatus = 'error';
                        return;
                    }}
                    this.ingesting = true; this.result = '';
                    const tags = this.tags ? this.tags.split(',').map(t=>t.trim()).filter(Boolean) : [];
                    const metadata = {{}};
                    if (this.title) metadata.title = this.title;
                    try {{
                        const r = await fetch('/api/v1/ingest', {{
                            method:'POST',
                            headers:{{'Content-Type':'application/json'}},
                            body: JSON.stringify({{
                                source: this.source,
                                content: this.content,
                                tags: tags,
                                metadata: metadata
                            }})
                        }});
                        const d = await r.json();
                        if (d.id) {{
                            this.result = d.id;
                            this.resultStatus = 'success';
                            this.history.unshift({{id:d.id, source:this.source, status:d.status}});
                        }} else {{
                            this.result = 'Ingest failed';
                            this.resultStatus = 'error';
                        }}
                    }} catch(e) {{ this.result = e.message; this.resultStatus = 'error'; }}
                    this.ingesting = false;
                }},
                clearForm() {{
                    this.source = ''; this.title = ''; this.content = ''; this.tags = '';
                    this.result = ''; this.resultStatus = '';
                }},
                async triggerSync() {{
                    this.syncing = true; this.syncResult = '';
                    try {{
                        let r;
                        if (this.syncSource) {{
                            r = await fetch('/api/v1/sync/' + this.syncSource, {{method:'POST'}});
                        }} else {{
                            r = await fetch('/api/v1/sync/trigger', {{method:'POST'}});
                        }}
                        const d = await r.json();
                        this.syncResult = d.status === 'completed'
                            ? 'Sync completed (task: ' + (d.task_id||'N/A') + ')'
                            : d.message || d.status || 'Sync triggered';
                        this.syncResultStatus = d.status === 'not_configured' ? 'error' : 'success';
                    }} catch(e) {{ this.syncResult = e.message; this.syncResultStatus = 'error'; }}
                    this.syncing = false;
                }}
            }}
        }}
    </script>
</body>
</html>"##,
        style = style_css(),
        nav = nav_html("ingest"),
    )
}

/// 状态监控页面
pub fn status_page(metrics_json: &str, monitor_json: &str, storage_json: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenMind - Status & Monitor</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
    <style>{style}</style>
</head>
<body>
    {nav}
    <div class="container" x-data="statusPage()">
        <!-- Health Overview -->
        <div class="stats-grid" style="margin-bottom:1rem">
            <div class="card stat">
                <div class="number" x-text="api.total_requests">0</div>
                <div class="label">Total Requests</div>
            </div>
            <div class="card stat">
                <div class="number health-ok" x-text="api.successful_requests">0</div>
                <div class="label">Successful</div>
            </div>
            <div class="card stat">
                <div class="number" :class="api.failed_requests>0?'health-err':''" x-text="api.failed_requests">0</div>
                <div class="label">Failed</div>
            </div>
            <div class="card stat">
                <div class="number" x-text="errorRate + '%'">0%</div>
                <div class="label">Error Rate</div>
            </div>
        </div>

        <div class="two-col">
            <!-- API Metrics -->
            <div class="card">
                <h2>📡 API Call Statistics</h2>
                <div style="margin-bottom:1rem">
                    <div style="display:flex;justify-content:space-between;font-size:0.85rem;margin-bottom:0.3rem">
                        <span>Success Rate</span>
                        <span x-text="successRate.toFixed(1) + '%'"></span>
                    </div>
                    <div class="progress-bar">
                        <div class="fill" :style="'width:' + successRate + '%'"></div>
                    </div>
                </div>
                <table>
                    <thead>
                        <tr><th>Endpoint</th><th>Calls</th><th>Success</th><th>Fail</th><th>Avg(ms)</th></tr>
                    </thead>
                    <tbody>
                        <template x-for="[ep, m] in Object.entries(api.by_endpoint)" :key="ep">
                            <tr>
                                <td x-text="ep"></td>
                                <td x-text="m.request_count"></td>
                                <td class="health-ok" x-text="m.success_count"></td>
                                <td :class="m.fail_count>0?'health-err':''" x-text="m.fail_count"></td>
                                <td x-text="m.request_count ? (m.total_response_time_ms / m.request_count).toFixed(0) : 0"></td>
                            </tr>
                        </template>
                    </tbody>
                </table>
                <div style="margin-top:0.5rem;font-size:0.8rem;color:var(--dim)">
                    Avg total response: <span x-text="api.total_requests ? (api.total_response_time_ms / api.total_requests).toFixed(0) : 0"></span>ms
                    | Uptime since: <span x-text="api.started_at"></span>
                </div>
            </div>

            <!-- Storage & Sync -->
            <div>
                <div class="card">
                    <h2>💾 Storage Usage</h2>
                    <table>
                        <tbody>
                            <tr><td>Total Entries</td><td x-text="storage.total_entries">0</td></tr>
                            <tr><td>Total Relations</td><td x-text="storage.total_relations">0</td></tr>
                            <tr><td>DB Size</td><td x-text="formatBytes(storage.db_size_bytes)">-</td></tr>
                        </tbody>
                    </table>
                    <div style="margin-top:0.8rem">
                        <div style="font-size:0.85rem;color:var(--dim);margin-bottom:0.3rem">Embedding Status</div>
                        <template x-for="[status, count] in Object.entries(storage.embedding_stats||{{}})" :key="status">
                            <span class="tag" x-text="status + ': ' + count"></span>
                        </template>
                    </div>
                    <div style="margin-top:0.8rem">
                        <div style="font-size:0.85rem;color:var(--dim);margin-bottom:0.3rem">By Source</div>
                        <template x-for="[src, count] in Object.entries(storage.source_stats||{{}})" :key="src">
                            <span class="tag" x-text="src + ': ' + count"></span>
                        </template>
                    </div>
                </div>

                <div class="card">
                    <h2>🔄 Sync Monitor</h2>
                    <div x-show="syncMetrics" style="margin-bottom:0.5rem">
                        <div style="display:flex;justify-content:space-between;font-size:0.85rem">
                            <span>Total Syncs</span><span x-text="syncMetrics?.total_syncs || 0"></span>
                        </div>
                        <div style="display:flex;justify-content:space-between;font-size:0.85rem">
                            <span>Successful</span><span class="health-ok" x-text="syncMetrics?.successful_syncs || 0"></span>
                        </div>
                        <div style="display:flex;justify-content:space-between;font-size:0.85rem">
                            <span>Failed</span><span :class="(syncMetrics?.failed_syncs||0)>0?'health-err':''" x-text="syncMetrics?.failed_syncs || 0"></span>
                        </div>
                        <div style="display:flex;justify-content:space-between;font-size:0.85rem">
                            <span>Items Synced</span><span x-text="syncMetrics?.total_items_synced || 0"></span>
                        </div>
                        <div style="display:flex;justify-content:space-between;font-size:0.85rem">
                            <span>Conflicts</span><span :class="(syncMetrics?.total_conflicts||0)>0?'health-warn':''" x-text="syncMetrics?.total_conflicts || 0"></span>
                        </div>
                    </div>
                    <div x-show="!syncMetrics" class="loading">Sync monitoring not configured</div>
                </div>
            </div>
        </div>
    </div>
    <script>
        function statusPage() {{
            return {{
                api: {api},
                storage: {storage},
                syncMetrics: {sync_metrics},
                get errorRate() {{
                    return this.api.total_requests
                        ? (this.api.failed_requests / this.api.total_requests * 100).toFixed(1)
                        : '0.0';
                }},
                get successRate() {{
                    return this.api.total_requests
                        ? (this.api.successful_requests / this.api.total_requests * 100)
                        : 100;
                }},
                formatBytes(bytes) {{
                    if (!bytes) return '-';
                    const units = ['B', 'KB', 'MB', 'GB'];
                    let i = 0;
                    while (bytes >= 1024 && i < 3) {{ bytes /= 1024; i++; }}
                    return bytes.toFixed(1) + ' ' + units[i];
                }}
            }}
        }}
    </script>
</body>
</html>"##,
        style = style_css(),
        nav = nav_html("status"),
        api = metrics_json,
        storage = storage_json,
        sync_metrics = monitor_json,
    )
}

/// 同步页面
pub fn sync_page(sync_status_json: &str, sync_metrics_json: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenMind - Sync</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
    <style>{style}</style>
</head>
<body>
    {nav}
    <div class="container" x-data="syncPage()">
        <div class="stats-grid" style="margin-bottom:1rem">
            <div class="card stat">
                <div class="number" x-text="status.active_tasks || 0">0</div>
                <div class="label">Active Tasks</div>
            </div>
            <div class="card stat">
                <div class="number" :class="status.scheduler_running?'health-ok':'health-warn'" x-text="status.scheduler_running?'Running':'Stopped'">-</div>
                <div class="label">Scheduler</div>
            </div>
            <div class="card stat">
                <div class="number" x-text="metrics.total_syncs || 0">0</div>
                <div class="label">Total Syncs</div>
            </div>
            <div class="card stat">
                <div class="number" x-text="metrics.total_items_synced || 0">0</div>
                <div class="label">Items Synced</div>
            </div>
        </div>

        <div class="two-col">
            <!-- Connector Status -->
            <div class="card">
                <h2>🔌 Connector Status</h2>
                <table>
                    <thead>
                        <tr><th>Connector</th><th>Status</th><th>Last Sync</th><th>Action</th></tr>
                    </thead>
                    <tbody>
                        <template x-for="[name, st] in Object.entries(status.connector_status||{{}})" :key="name">
                            <tr>
                                <td x-text="name"></td>
                                <td>
                                    <span class="badge" :class="st==='healthy'?'badge-ok':st==='error'?'badge-err':'badge-warn'" x-text="st"></span>
                                </td>
                                <td style="font-size:0.8rem;color:var(--dim)" x-text="status.last_sync[name]||'Never'"></td>
                                <td><button class="secondary" style="padding:0.3rem 0.6rem;font-size:0.75rem" @click="triggerSync(name)">Sync</button></td>
                            </tr>
                        </template>
                    </tbody>
                </table>
                <div x-show="Object.keys(status.connector_status||{{}}).length===0" class="loading">No connectors configured</div>
            </div>

            <!-- Per-Connector Metrics -->
            <div class="card">
                <h2>📊 Connector Metrics</h2>
                <template x-for="[name, m] in Object.entries(metrics.by_connector||{{}})" :key="name">
                    <div style="padding:0.8rem 0;border-bottom:1px solid var(--border)">
                        <div style="font-weight:600;margin-bottom:0.3rem" x-text="name"></div>
                        <div style="display:flex;gap:1rem;font-size:0.8rem;color:var(--dim)">
                            <span>Syncs: <span x-text="m.sync_count"></span></span>
                            <span class="health-ok">OK: <span x-text="m.success_count"></span></span>
                            <span :class="m.fail_count>0?'health-err':''">Fail: <span x-text="m.fail_count"></span></span>
                            <span>Items: <span x-text="m.items_synced"></span></span>
                        </div>
                        <div x-show="m.last_error" style="font-size:0.75rem;color:var(--err);margin-top:0.3rem" x-text="'Last error: ' + m.last_error"></div>
                    </div>
                </template>
                <div x-show="Object.keys(metrics.by_connector||{{}}).length===0" class="loading">No sync activity yet</div>
            </div>
        </div>

        <div style="margin-top:1rem">
            <button @click="triggerSyncAll()" :disabled="syncing">
                <span x-show="!syncing">⚡ Sync All Connectors</span>
                <span x-show="syncing">⏳ Syncing All...</span>
            </button>
            <span x-show="syncResult" style="margin-left:0.5rem" :class="syncResultOk?'badge badge-ok':'badge badge-err'" x-text="syncResult"></span>
        </div>
    </div>
    <script>
        function syncPage() {{
            return {{
                status: {status},
                metrics: {metrics},
                syncing: false,
                syncResult: '',
                syncResultOk: true,
                async triggerSync(connector) {{
                    try {{
                        const r = await fetch('/api/v1/sync/' + connector, {{method:'POST'}});
                        const d = await r.json();
                        alert(d.status === 'completed' ? 'Sync completed: ' + (d.task_id||'') : d.message || d.status);
                    }} catch(e) {{ alert('Sync failed: ' + e.message); }}
                }},
                async triggerSyncAll() {{
                    this.syncing = true; this.syncResult = '';
                    try {{
                        const r = await fetch('/api/v1/sync/trigger', {{method:'POST'}});
                        const d = await r.json();
                        this.syncResult = d.status || 'triggered';
                        this.syncResultOk = d.status !== 'not_configured';
                    }} catch(e) {{ this.syncResult = e.message; this.syncResultOk = false; }}
                    this.syncing = false;
                }}
            }}
        }}
    </script>
</body>
</html>"##,
        style = style_css(),
        nav = nav_html("sync"),
        status = sync_status_json,
        metrics = sync_metrics_json,
    )
}
