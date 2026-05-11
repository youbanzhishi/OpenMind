# OpenMind

> AI原生的个人知识引擎 — Agent生态中的知识节点

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## 是什么

OpenMind是一个知识引擎，不是文件管理器，不是搜索引擎，而是**Agent生态中的知识节点**。

- **语义搜索**：不只是关键词匹配，而是理解你的意图
- **知识关联**：自动发现知识之间的联系，构建知识图谱
- **多源同步**：从博客、备忘录、书签、Vault文件等来源摄入知识
- **RAG查询**：基于检索增强生成的知识问答
- **Agent Action Protocol**：通过标准化协议与其他Agent协作

## 架构原则

- **节点平等**：OpenMind是节点不是中枢，与OpenDAW、OpenLink等平等协作
- **协议驱动**：通过Agent Action Protocol定义交互契约
- **点对点协作**：任何Agent可直接调用OpenMind的Action
- **发现即接入**：通过`/.well-known/agent.json`自动发现能力

## 技术栈

- **Rust (Axum)** — 高性能HTTP服务
- **Qdrant** — 向量存储与语义检索
- **SQLite/PostgreSQL** — 元数据存储
- **可插拔嵌入模型** — OpenAI/本地模型

## 项目结构

```
OpenMind/
├── crates/
│   ├── openmind-core/    # 核心抽象：Connector/Storage/Embedding trait + 数据模型
│   ├── openmind-ingest/  # 摄入管道：解析+分块+嵌入
│   ├── openmind-search/  # 搜索引擎：关键词+语义+混合
│   ├── openmind-graph/   # 知识图谱：实体+关系
│   ├── openmind-api/     # HTTP API：Axum路由
│   └── openmind-cli/     # CLI工具
├── .well-known/
│   └── agent.json        # Agent发现协议
├── docs/                 # 文档
├── Dockerfile
└── docker-compose.yml
```

## 快速开始

### Docker Compose

```bash
git clone https://github.com/youbanzhishi/OpenMind.git
cd OpenMind
docker compose up -d
```

### 从源码

```bash
cargo build --release --bin openmind
./target/release/openmind
```

## API

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | /api/v1/search | 搜索（keyword/semantic/hybrid） |
| POST | /api/v1/ingest | 摄入内容 |
| GET | /api/v1/entry/:id | 获取知识条目 |
| GET | /api/v1/entry/:id/related | 获取关联知识 |
| POST | /api/v1/sync/:source | 触发同步 |
| GET | /api/v1/connectors | 列出已注册Connector |
| GET | /.well-known/agent.json | Agent发现 |
| GET | /api/v1/health | 健康检查 |

## Agent Action Protocol

OpenMind实现了Agent Action Protocol，其他Agent可以通过标准化契约调用：

```json
{
  "name": "semantic_search",
  "description": "语义搜索知识库",
  "endpoint": "POST /api/v1/search",
  "input": { "query": "string", "mode": "keyword|semantic|hybrid", "limit": 10 },
  "output": { "results": [{ "content": "string", "source": "string", "relevance": 0.0 }] }
}
```

详见 [Agent集成指南](docs/agent-guide.md)。

## 存储策略

- **文本内容**：直接存入OpenMind数据层（可搜索、可嵌入）
- **大文件**（图片/音频/视频）：只存引用指针，原始文件保留在OpenVault/S3

## License

MIT
