//! 嵌入模型实现
//!
//! 提供两种嵌入模型：
//! - OpenAIEmbeddingModel: 调用OpenAI text-embedding-3-small API
//! - DummyEmbeddingModel: 测试用，返回随机向量

use crate::traits::EmbeddingModel;
use async_trait::async_trait;
use serde::Deserialize;

/// OpenAI嵌入模型
///
/// 调用text-embedding-3-small API生成嵌入向量。
pub struct OpenAIEmbeddingModel {
    api_key: String,
    model: String,
    dimension: usize,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl OpenAIEmbeddingModel {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "text-embedding-3-small".to_string(),
            dimension: 1536,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: String, dimension: usize) -> Self {
        self.model = model;
        self.dimension = dimension;
        self
    }
}

#[async_trait]
impl EmbeddingModel for OpenAIEmbeddingModel {
    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": text,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error {}: {}", status, body);
        }

        let result: EmbeddingResponse = response.json().await?;
        result
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| anyhow::anyhow!("No embedding returned"))
    }

    async fn health_check(&self) -> bool {
        // Try embedding a tiny string
        match self.embed_text("test").await {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!("OpenAI embedding health check failed: {}", e);
                false
            }
        }
    }
}

/// 测试用Dummy嵌入模型
///
/// 返回随机向量，不调用任何外部API。
pub struct DummyEmbeddingModel {
    dimension: usize,
}

impl DummyEmbeddingModel {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

#[async_trait]
impl EmbeddingModel for DummyEmbeddingModel {
    fn model_name(&self) -> &str {
        "dummy"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn embed_text(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        // Deterministic pseudo-random based on simple hash
        let mut vec = Vec::with_capacity(self.dimension);
        for i in 0..self.dimension {
            let val = ((i as u64).wrapping_mul(2654435761) % 1000) as f32 / 1000.0;
            vec.push(val);
        }
        Ok(vec)
    }

    async fn health_check(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dummy_embedding() {
        let model = DummyEmbeddingModel::new(128);
        assert_eq!(model.model_name(), "dummy");
        assert_eq!(model.dimension(), 128);

        let embedding = model.embed_text("hello world").await.unwrap();
        assert_eq!(embedding.len(), 128);

        // Deterministic: same input → same output
        let embedding2 = model.embed_text("hello world").await.unwrap();
        assert_eq!(embedding, embedding2);
    }

    #[tokio::test]
    async fn test_dummy_health_check() {
        let model = DummyEmbeddingModel::new(64);
        assert!(model.health_check().await);
    }
}
