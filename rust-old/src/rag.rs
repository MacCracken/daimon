//! Retrieval-Augmented Generation (RAG) pipeline.
//!
//! Text chunking, bag-of-words embedding, and a pipeline that combines
//! [`VectorIndex`] retrieval with context
//! formatting for LLM injection.
//!
//! The embedding strategy (`simple_embed`) is a placeholder TF approach —
//! adequate for keyword-overlap retrieval until a real model is integrated.

use std::collections::{BTreeMap, HashMap};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

use crate::error::{DaimonError, Result};
use crate::vector_store::{VectorEntry, VectorIndex};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the RAG pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RagConfig {
    /// Number of nearest chunks to retrieve per query.
    pub top_k: usize,
    /// Maximum characters per chunk when splitting input text.
    pub chunk_size: usize,
    /// Overlapping characters between consecutive chunks.
    pub overlap: usize,
    /// Minimum cosine similarity score to include a chunk.
    pub min_relevance_score: f64,
    /// Template for formatted context (`{context}` and `{query}` are replaced).
    pub context_template: String,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            chunk_size: 512,
            overlap: 64,
            min_relevance_score: 0.1,
            context_template:
                "Use the following context to answer the question.\n\n---\n{context}\n---\n\nQuestion: {query}"
                    .to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// A single chunk retrieved by the RAG pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RetrievedChunk {
    /// The textual content of the chunk.
    pub content: String,
    /// Cosine similarity score to the query.
    pub score: f64,
    /// Arbitrary metadata carried over from the vector entry.
    pub metadata: serde_json::Value,
}

/// Full RAG query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RagContext {
    /// Retrieved chunks, sorted by descending relevance.
    pub chunks: Vec<RetrievedChunk>,
    /// Ready-to-inject context string built from the template.
    pub formatted_context: String,
    /// Rough token estimate (chars / 4).
    pub total_tokens_estimate: usize,
}

// ---------------------------------------------------------------------------
// Embedding helpers
// ---------------------------------------------------------------------------

/// Build vocabulary from existing index entries.
#[must_use]
fn build_vocab(index: &VectorIndex) -> Vec<String> {
    let mut vocab_set: BTreeMap<String, ()> = BTreeMap::new();
    for entry in index.entries() {
        for word in tokenize(&entry.content) {
            vocab_set.entry(word).or_default();
        }
    }
    vocab_set.into_keys().collect()
}

/// Tokenize text into lowercased alphanumeric words.
#[must_use]
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Simple bag-of-words term-frequency embedding against a vocabulary.
#[must_use]
pub fn simple_embed(text: &str, vocab: &[String]) -> Vec<f64> {
    if vocab.is_empty() {
        return Vec::new();
    }
    let tokens = tokenize(text);
    let total = tokens.len().max(1) as f64;

    let mut freq: HashMap<&str, usize> = HashMap::new();
    for t in &tokens {
        *freq.entry(t.as_str()).or_default() += 1;
    }

    vocab
        .iter()
        .map(|word| *freq.get(word.as_str()).unwrap_or(&0) as f64 / total)
        .collect()
}

/// Self-contained embedding for a single text (builds vocab from text itself).
#[must_use]
pub fn simple_embed_standalone(text: &str) -> Vec<f64> {
    let tokens = tokenize(text);
    let mut vocab_set: BTreeMap<String, ()> = BTreeMap::new();
    for t in &tokens {
        vocab_set.entry(t.clone()).or_default();
    }
    let vocab: Vec<String> = vocab_set.into_keys().collect();
    simple_embed(text, &vocab)
}

// ---------------------------------------------------------------------------
// Text chunking
// ---------------------------------------------------------------------------

/// Split `text` into chunks of at most `chunk_size` characters with `overlap`
/// characters shared between consecutive chunks.
#[must_use]
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() || chunk_size == 0 {
        return Vec::new();
    }

    let overlap = overlap.min(chunk_size.saturating_sub(1));
    let step = chunk_size - overlap;

    let char_boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let char_count = char_boundaries.len();

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < char_count {
        let end = (start + chunk_size).min(char_count);
        let byte_start = char_boundaries[start];
        let byte_end = if end < char_count {
            char_boundaries[end]
        } else {
            text.len()
        };
        chunks.push(text[byte_start..byte_end].to_string());
        start += step;
    }

    chunks
}

// ---------------------------------------------------------------------------
// RagPipeline
// ---------------------------------------------------------------------------

/// Retrieval-augmented generation pipeline.
///
/// Wraps a [`VectorIndex`] and provides chunking, ingestion, and formatted
/// retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RagPipeline {
    /// The underlying vector index.
    pub index: VectorIndex,
    /// Pipeline configuration.
    pub config: RagConfig,
    vocab_cache: BTreeMap<String, ()>,
}

impl RagPipeline {
    /// Create a new pipeline with the given config.
    #[must_use]
    pub fn new(config: RagConfig) -> Self {
        Self {
            index: VectorIndex::new(),
            config,
            vocab_cache: BTreeMap::new(),
        }
    }

    /// Ingest a text document: chunk, embed, and insert into the index.
    pub fn ingest_text(&mut self, text: &str, metadata: serde_json::Value) -> Result<Vec<Uuid>> {
        let chunks = chunk_text(text, self.config.chunk_size, self.config.overlap);
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let old_vocab_size = self.vocab_cache.len();
        for c in &chunks {
            for w in tokenize(c) {
                self.vocab_cache.entry(w).or_default();
            }
        }
        let vocab: Vec<String> = self.vocab_cache.keys().cloned().collect();

        let new_dim = vocab.len();
        let vocab_grew = old_vocab_size > 0 && new_dim != old_vocab_size;

        if vocab_grew {
            let old_entries: Vec<VectorEntry> = self.index.entries().cloned().collect();
            let mut new_index = VectorIndex::new();
            for mut e in old_entries {
                e.embedding = simple_embed(&e.content, &vocab);
                new_index.insert(e).map_err(|e| {
                    DaimonError::StorageError(format!("re-embed existing entry: {e}"))
                })?;
            }
            self.index = new_index;
        }

        let mut ids = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let embedding = simple_embed(chunk, &vocab);
            let entry = VectorEntry {
                id: Uuid::new_v4(),
                embedding,
                metadata: metadata.clone(),
                content: chunk.clone(),
                created_at: Utc::now(),
            };
            let id = self
                .index
                .insert(entry)
                .map_err(|e| DaimonError::StorageError(format!("insert chunk: {e}")))?;
            ids.push(id);
        }

        debug!(
            chunks = ids.len(),
            vocab_size = vocab.len(),
            "ingested text"
        );
        Ok(ids)
    }

    /// Query the pipeline: retrieve top-k relevant chunks and format context.
    #[must_use]
    pub fn query(&self, query_embedding: &[f64], user_query: &str) -> RagContext {
        let results = self.index.search(query_embedding, self.config.top_k);

        let chunks: Vec<RetrievedChunk> = results
            .into_iter()
            .filter(|r| r.score >= self.config.min_relevance_score)
            .map(|r| RetrievedChunk {
                content: r.content,
                score: r.score,
                metadata: r.metadata,
            })
            .collect();

        let context_body: String = chunks
            .iter()
            .enumerate()
            .map(|(i, c)| format!("[{}] (score: {:.3}) {}", i + 1, c.score, c.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let formatted_context = self
            .config
            .context_template
            .replace("{context}", &context_body)
            .replace("{query}", user_query);

        let total_tokens_estimate = formatted_context.len() / 4;

        RagContext {
            chunks,
            formatted_context,
            total_tokens_estimate,
        }
    }

    /// Convenience: embed a query string using current vocabulary, then query.
    #[must_use]
    pub fn query_text(&self, user_query: &str) -> RagContext {
        let vocab = build_vocab(&self.index);
        let embedding = simple_embed(user_query, &vocab);
        self.query(&embedding, user_query)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- chunk_text --

    #[test]
    fn chunk_no_overlap() {
        assert_eq!(chunk_text("abcdefgh", 4, 0), vec!["abcd", "efgh"]);
    }

    #[test]
    fn chunk_with_overlap() {
        assert_eq!(
            chunk_text("abcdefgh", 4, 2),
            vec!["abcd", "cdef", "efgh", "gh"]
        );
    }

    #[test]
    fn chunk_overlap_clamped() {
        let chunks = chunk_text("abcd", 3, 100);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0], "abc");
    }

    #[test]
    fn chunk_empty_text() {
        assert!(chunk_text("", 10, 0).is_empty());
    }

    #[test]
    fn chunk_zero_size() {
        assert!(chunk_text("hello", 0, 0).is_empty());
    }

    #[test]
    fn chunk_size_larger_than_text() {
        assert_eq!(chunk_text("hi", 100, 0), vec!["hi"]);
    }

    #[test]
    fn chunk_multibyte_unicode() {
        let text = "\u{1F600}\u{1F601}\u{1F602}\u{1F603}\u{1F604}\u{1F605}";
        let chunks = chunk_text(text, 3, 1);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "\u{1F600}\u{1F601}\u{1F602}");
    }

    // -- tokenize --

    #[test]
    fn tokenize_basic() {
        assert_eq!(
            tokenize("Hello, World! Test-123."),
            vec!["hello", "world", "test", "123"]
        );
    }

    #[test]
    fn tokenize_empty() {
        assert!(tokenize("").is_empty());
    }

    // -- simple_embed --

    #[test]
    fn embed_basic() {
        let vocab = vec!["cat".into(), "dog".into(), "fish".into()];
        let emb = simple_embed("cat cat dog", &vocab);
        assert_eq!(emb.len(), 3);
        assert!((emb[0] - 2.0 / 3.0).abs() < 1e-9);
        assert!((emb[1] - 1.0 / 3.0).abs() < 1e-9);
        assert!(emb[2].abs() < 1e-9);
    }

    #[test]
    fn embed_empty_vocab() {
        assert!(simple_embed("hello", &[]).is_empty());
    }

    #[test]
    fn embed_standalone() {
        let emb = simple_embed_standalone("hello world hello");
        assert_eq!(emb.len(), 2);
    }

    // -- RagPipeline --

    #[test]
    fn pipeline_ingest_and_query() {
        let config = RagConfig {
            top_k: 2,
            chunk_size: 50,
            overlap: 0,
            min_relevance_score: 0.0,
            ..Default::default()
        };
        let mut pipeline = RagPipeline::new(config);

        let ids = pipeline
            .ingest_text(
                "The cat sat on the mat. The dog played in the yard.",
                json!({"doc": 1}),
            )
            .unwrap();
        assert!(!ids.is_empty());

        let ctx = pipeline.query_text("cat mat");
        assert!(!ctx.chunks.is_empty());
        assert!(ctx.formatted_context.contains("cat mat"));
        assert!(ctx.total_tokens_estimate > 0);
    }

    #[test]
    fn pipeline_empty_ingest() {
        let mut pipeline = RagPipeline::new(RagConfig::default());
        assert!(pipeline.ingest_text("", json!({})).unwrap().is_empty());
    }

    #[test]
    fn pipeline_min_relevance_filters() {
        let config = RagConfig {
            top_k: 10,
            chunk_size: 1000,
            overlap: 0,
            min_relevance_score: 0.99,
            ..Default::default()
        };
        let mut pipeline = RagPipeline::new(config);
        pipeline
            .ingest_text("alpha beta gamma delta", json!({}))
            .unwrap();

        let ctx = pipeline.query_text("zzzzz xxxx yyyy");
        assert!(ctx.formatted_context.contains("zzzzz xxxx yyyy"));
    }

    #[test]
    fn pipeline_multiple_ingests() {
        let config = RagConfig {
            top_k: 5,
            chunk_size: 100,
            overlap: 0,
            min_relevance_score: 0.0,
            ..Default::default()
        };
        let mut pipeline = RagPipeline::new(config);

        pipeline
            .ingest_text("rust programming language", json!({"batch": 1}))
            .unwrap();
        pipeline
            .ingest_text("python scripting language", json!({"batch": 2}))
            .unwrap();

        let ctx = pipeline.query_text("rust");
        assert!(!ctx.chunks.is_empty());
        assert!(ctx.chunks[0].content.contains("rust"));
    }

    #[test]
    fn pipeline_custom_template() {
        let config = RagConfig {
            top_k: 1,
            chunk_size: 1000,
            overlap: 0,
            min_relevance_score: 0.0,
            context_template: "CTX:{context} Q:{query}".to_string(),
        };
        let mut pipeline = RagPipeline::new(config);
        pipeline.ingest_text("hello world", json!({})).unwrap();

        let ctx = pipeline.query_text("hello");
        assert!(ctx.formatted_context.starts_with("CTX:"));
        assert!(ctx.formatted_context.contains("Q:hello"));
    }

    #[test]
    fn pipeline_query_empty_index() {
        let pipeline = RagPipeline::new(RagConfig::default());
        assert!(pipeline.query_text("anything").chunks.is_empty());
    }

    #[test]
    fn pipeline_metadata_preserved() {
        let config = RagConfig {
            top_k: 1,
            chunk_size: 1000,
            overlap: 0,
            min_relevance_score: 0.0,
            ..Default::default()
        };
        let mut pipeline = RagPipeline::new(config);
        pipeline
            .ingest_text("specific content here", json!({"source": "test_doc"}))
            .unwrap();

        let ctx = pipeline.query_text("specific content");
        assert!(!ctx.chunks.is_empty());
        assert_eq!(ctx.chunks[0].metadata["source"], "test_doc");
    }

    // -- serde roundtrips --

    #[test]
    fn rag_config_serde_roundtrip() {
        let cfg = RagConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: RagConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.top_k, cfg.top_k);
        assert_eq!(back.chunk_size, cfg.chunk_size);
    }

    #[test]
    fn rag_context_serde_roundtrip() {
        let ctx = RagContext {
            chunks: vec![RetrievedChunk {
                content: "hello".into(),
                score: 0.95,
                metadata: json!({}),
            }],
            formatted_context: "formatted".into(),
            total_tokens_estimate: 10,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: RagContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.chunks.len(), 1);
        assert_eq!(back.total_tokens_estimate, 10);
    }

    #[test]
    fn rag_pipeline_serde_roundtrip() {
        let mut pipeline = RagPipeline::new(RagConfig::default());
        pipeline
            .ingest_text("hello world test data", json!({}))
            .unwrap();
        let json = serde_json::to_string(&pipeline).unwrap();
        let back: RagPipeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back.index.len(), pipeline.index.len());
        assert_eq!(back.config.top_k, pipeline.config.top_k);
    }
}
