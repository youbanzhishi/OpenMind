# OpenMind 用户指南

## 概述

OpenMind 是一个AI原生的个人知识引擎，作为Agent生态中的知识节点运行。

## 核心概念

### 知识条目 (Knowledge Entry)
知识库的基本单元，包含文本内容和元数据。每个条目有唯一的来源标识（source），如 `vault:notes/daily.md`。

### 数据源 (Connector)
OpenMind通过Connector从不同来源摄入内容：
- **Vault Connector**: 从OpenVault同步文件
- **Blog Connector**: 从博客导入文章
- **Bookmark Connector**: 导入书签
- **Note Connector**: 同步备忘录

### 搜索模式
- **关键词搜索 (Keyword)**: 基于全文索引的精确匹配
- **语义搜索 (Semantic)**: 基于向量嵌入的语义相似度
- **混合搜索 (Hybrid)**: 融合关键词和语义搜索结果

### 存储策略
- 文本内容直接存入OpenMind数据层（可搜索）
- 大文件（图片/音频/视频）只存引用指针，原始文件保留在OpenVault/S3

## API 快速开始

### 健康检查
```bash
curl http://localhost:9090/api/v1/health
```

### 搜索
```bash
curl -X POST http://localhost:9090/api/v1/search \
  -H "Content-Type: application/json" \
  -d '{"query": "Rust异步编程", "mode": "hybrid", "limit": 10}'
```

### 摄入内容
```bash
curl -X POST http://localhost:9090/api/v1/ingest \
  -H "Content-Type: application/json" \
  -d '{"source": "manual", "content": "学习笔记内容...", "metadata": {}, "tags": ["rust", "async"]}'
```

### 触发同步
```bash
curl -X POST http://localhost:9090/api/v1/sync/vault
```
