# OpenMind 部署指南

## Docker Compose（推荐）

```bash
docker compose up -d
```

启动后：
- OpenMind API: http://localhost:9090
- Qdrant Dashboard: http://localhost:6333/dashboard

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| QDRANT_URL | http://qdrant:6333 | Qdrant向量数据库地址 |
| DATABASE_URL | sqlite:///data/openmind.db | 元数据数据库连接 |
| RUST_LOG | info | 日志级别 |
| PORT | 9090 | 服务端口 |
| EMBEDDING_MODEL | openai | 嵌入模型（openai/local） |
| OPENAI_API_KEY | - | OpenAI API密钥（使用openai嵌入时需要） |

## 从源码编译

```bash
# 需要Rust 1.75+
cargo build --release --bin openmind

# 运行
./target/release/openmind
```

## 数据持久化

Docker Compose配置了两个volume：
- `openmind-data`: SQLite数据库
- `qdrant-data`: 向量索引数据

## 备份

```bash
# 备份SQLite
cp /var/lib/docker/volumes/openmind_openmind-data/_data/openmind.db ./backup/

# 备份Qdrant快照
curl -X POST http://localhost:6333/snapshots
```

## ECS部署

参考项目 `scripts/deploy.sh` 和 `docs/knowledge/deploy.md`。
