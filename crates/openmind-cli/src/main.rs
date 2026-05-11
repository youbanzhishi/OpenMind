//! OpenMind CLI - 命令行入口
//!
//! 支持以下命令：
//! - `openmind serve` — 启动API服务器
//! - `openmind ingest <path>` — 摄入文件/目录
//! - `openmind search <query>` — 搜索
//! - `openmind status` — 知识库统计

use openmind_api::{create_router, AppState};
use openmind_core::{
    ContentItem, DummyEmbeddingModel, IngestionPipeline, KnowledgeStore, SearchFilters,
    SqliteKnowledgeStore,
};
use openmind_ingest::DefaultIngestionPipeline;
use std::path::Path;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

const DEFAULT_DB_PATH: &str = "openmind.db";
const DEFAULT_PORT: u16 = 9090;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("serve");

    match command {
        "serve" => cmd_serve().await,
        "ingest" => {
            let path = args.get(2).map(|s| s.as_str()).unwrap_or(".");
            cmd_ingest(path).await
        }
        "search" => {
            let query = args.get(2).map(|s| s.as_str()).unwrap_or("");
            cmd_search(query).await
        }
        "status" => cmd_status().await,
        _ => {
            eprintln!("OpenMind - AI-native personal knowledge engine");
            eprintln!();
            eprintln!("Usage: openmind <command> [args]");
            eprintln!();
            eprintln!("Commands:");
            eprintln!("  serve            Start the API server (default)");
            eprintln!("  ingest <path>    Ingest a file or directory");
            eprintln!("  search <query>   Search the knowledge base");
            eprintln!("  status           Show knowledge base statistics");
            std::process::exit(1);
        }
    }
}

async fn cmd_serve() -> anyhow::Result<()> {
    let store = SqliteKnowledgeStore::open(DEFAULT_DB_PATH)?;
    let state = Arc::new(AppState::new(store));

    let app = create_router(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT));
    tracing::info!("OpenMind API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn cmd_ingest(path: &str) -> anyhow::Result<()> {
    let store = SqliteKnowledgeStore::open(DEFAULT_DB_PATH)?;
    let embedding = DummyEmbeddingModel::new(64);
    let pipeline = DefaultIngestionPipeline::new(embedding, store);

    let path_obj = Path::new(path);

    if path_obj.is_file() {
        ingest_file(&pipeline, path_obj).await?;
    } else if path_obj.is_dir() {
        let mut count = 0;
        let mut entries = std::fs::read_dir(path_obj)?;
        while let Some(entry) = entries.next() {
            let entry = entry?;
            let p = entry.path();
            if p.is_file() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(
                    ext,
                    "md" | "txt" | "html" | "htm" | "rs" | "py" | "js" | "ts"
                ) {
                    match ingest_file(&pipeline, &p).await {
                        Ok(ids) => {
                            count += ids.len();
                            tracing::info!("Ingested {} chunks from {:?}", ids.len(), p);
                        }
                        Err(e) => {
                            tracing::error!("Failed to ingest {:?}: {}", p, e);
                        }
                    }
                }
            }
        }
        tracing::info!("Ingestion complete: {} total chunks", count);
    } else {
        anyhow::bail!("Path not found: {}", path);
    }

    Ok(())
}

async fn ingest_file(
    pipeline: &DefaultIngestionPipeline<DummyEmbeddingModel, SqliteKnowledgeStore>,
    path: &Path,
) -> anyhow::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let source = path.to_string_lossy().to_string();
    let content_type = infer_content_type(&source);

    let item = ContentItem {
        source,
        content_type,
        content,
        title: Some(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        ),
        metadata: serde_json::json!({}),
        file_references: vec![],
        tags: vec![],
    };

    pipeline.ingest(item).await
}

fn infer_content_type(source: &str) -> String {
    if source.ends_with(".md") || source.ends_with(".markdown") {
        "markdown".to_string()
    } else if source.ends_with(".html") || source.ends_with(".htm") {
        "html".to_string()
    } else if source.ends_with(".rs")
        || source.ends_with(".py")
        || source.ends_with(".js")
        || source.ends_with(".ts")
    {
        "code".to_string()
    } else {
        "text".to_string()
    }
}

async fn cmd_search(query: &str) -> anyhow::Result<()> {
    if query.is_empty() {
        anyhow::bail!("Search query cannot be empty");
    }

    let store = SqliteKnowledgeStore::open(DEFAULT_DB_PATH)?;
    let filters = SearchFilters::default();
    let results = store.query_keyword(query, 10, &filters).await?;

    if results.is_empty() {
        println!("No results found for: {}", query);
        return Ok(());
    }

    println!("Search results for '{}' ({} found):", query, results.len());
    println!("{}", "-".repeat(60));

    for (i, result) in results.iter().enumerate() {
        println!(
            "\n{}. [{}] {}",
            i + 1,
            result.entry.source_type.as_str(),
            result.entry.title
        );
        println!("   Source: {}", result.entry.source_id);
        println!("   Relevance: {:.3}", result.relevance);
        if !result.entry.tags.is_empty() {
            println!("   Tags: {}", result.entry.tags.join(", "));
        }
        // Show a snippet
        let snippet: String = result.entry.content.chars().take(200).collect();
        println!("   {}", snippet);
        if result.entry.content.len() > 200 {
            println!("   ...");
        }
    }

    Ok(())
}

async fn cmd_status() -> anyhow::Result<()> {
    let store = SqliteKnowledgeStore::open(DEFAULT_DB_PATH)?;
    let stats = store.stats().await?;

    println!("OpenMind Knowledge Base Statistics");
    println!("{}", "=".repeat(40));
    println!("Total entries:   {}", stats.total_entries);
    println!("Total relations: {}", stats.total_relations);
    println!("Total tags:      {}", stats.total_tags);

    if stats.total_entries > 0 {
        println!("\nBy source type:");
        if let Some(obj) = stats.by_source.as_object() {
            for (k, v) in obj {
                println!("  {}: {}", k, v);
            }
        }

        println!("\nBy embedding status:");
        if let Some(obj) = stats.by_embedding_status.as_object() {
            for (k, v) in obj {
                println!("  {}: {}", k, v);
            }
        }
    }

    Ok(())
}
