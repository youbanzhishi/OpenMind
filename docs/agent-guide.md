# OpenMind Agent 集成指南

## Agent发现

OpenMind通过 `/.well-known/agent.json` 暴露Agent能力描述。

```bash
curl http://localhost:9090/.well-known/agent.json
```

## Action Protocol

OpenMind实现了Agent Action Protocol，定义了清晰的输入输出契约：

### semantic_search
- **端点**: `POST /api/v1/search`
- **输入**: `{ "query": "string", "mode": "keyword|semantic|hybrid", "limit": 10, "filters": {} }`
- **输出**: `{ "results": [{ "content": "string", "source": "string", "relevance": 0.0, "highlights": [] }] }`

### find_todos
- **端点**: `POST /api/v1/search`
- **输入**: `{ "query": "string", "filters": { "type": "todo" } }`
- **输出**: `{ "results": [{ "content": "string", "files": ["string"], "project": "string" }] }`

### ingest
- **端点**: `POST /api/v1/ingest`
- **输入**: `{ "source": "string", "content": "string", "metadata": {} }`
- **输出**: `{ "id": "string", "status": "indexed" }`

### get_related
- **端点**: `GET /api/v1/entry/{id}/related`
- **输入**: `{ "id": "string", "depth": 1 }`
- **输出**: `{ "relations": [{ "entry_id": "string", "relation_type": "string", "weight": 0.0 }] }`

## 工作流编排示例

### search_and_mix 工作流
1. OpenMind: `semantic_search` 查找相关知识
2. OpenDAW: 基于知识内容生成音频
3. OpenLink: 发布到互联网

### find_and_act 工作流
1. OpenMind: `find_todos` 查找待办事项
2. 其他Agent: 根据待办内容执行操作
