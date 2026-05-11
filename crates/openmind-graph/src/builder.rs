//! 知识图谱构建器
//!
//! 基于语义相似度和元数据关联构建知识图谱。

use openmind_core::{KnowledgeEntry, KnowledgeRelation, KnowledgeStore};

/// 知识图谱构建器
pub struct GraphBuilder<K: KnowledgeStore> {
    store: K,
}

impl<K: KnowledgeStore> GraphBuilder<K> {
    pub fn new(store: K) -> Self {
        Self { store }
    }

    /// 基于语义相似度建立关联
    ///
    /// 当两个知识条目的语义相似度超过阈值时，自动建立"similar_to"关联。
    pub async fn build_similarity_relations(
        &self,
        entries: &[KnowledgeEntry],
        threshold: f64,
    ) -> anyhow::Result<Vec<KnowledgeRelation>> {
        let mut relations = Vec::new();

        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                // Similarity check would use embedding vectors
                // For now, create a placeholder relation structure
                let relation = KnowledgeRelation {
                    from_id: entries[i].id.clone(),
                    to_id: entries[j].id.clone(),
                    relation_type: "similar_to".to_string(),
                    weight: 0.0, // Would be computed from cosine similarity
                };

                if relation.weight >= threshold {
                    self.store.relate(relation.clone()).await?;
                    relations.push(relation);
                }
            }
        }

        Ok(relations)
    }

    /// 基于标签匹配建立关联
    pub async fn build_tag_relations(
        &self,
        entries: &[KnowledgeEntry],
    ) -> anyhow::Result<Vec<KnowledgeRelation>> {
        let mut relations = Vec::new();

        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let common_tags: Vec<_> = entries[i]
                    .tags
                    .iter()
                    .filter(|t| entries[j].tags.contains(t))
                    .collect();

                if !common_tags.is_empty() {
                    let weight = common_tags.len() as f64
                        / (entries[i].tags.len().max(entries[j].tags.len()) as f64);

                    let relation = KnowledgeRelation {
                        from_id: entries[i].id.clone(),
                        to_id: entries[j].id.clone(),
                        relation_type: "tagged_with".to_string(),
                        weight,
                    };

                    self.store.relate(relation.clone()).await?;
                    relations.push(relation);
                }
            }
        }

        Ok(relations)
    }

    /// 获取指定条目的关联图谱
    pub async fn get_graph(
        &self,
        entry_id: &str,
        depth: usize,
    ) -> anyhow::Result<Vec<KnowledgeRelation>> {
        self.store.get_related(entry_id, depth).await
    }
}
