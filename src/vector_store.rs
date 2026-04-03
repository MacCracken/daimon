//! Embedded vector store — in-memory cosine-similarity search with JSON persistence.
//!
//! No external ML or vector-search dependencies — built entirely on serde, uuid,
//! and chrono. Suitable for moderate-scale agent knowledge bases.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

use crate::error::{DaimonError, Result};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single vector entry stored in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VectorEntry {
    /// Unique identifier for this entry.
    pub id: Uuid,
    /// The embedding vector (all entries in an index share the same dimensionality).
    pub embedding: Vec<f64>,
    /// Arbitrary JSON metadata attached to this entry.
    pub metadata: serde_json::Value,
    /// The original textual content this vector represents.
    pub content: String,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
}

/// A search result returned by [`VectorIndex::search`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SearchResult {
    /// The matched entry's ID.
    pub id: Uuid,
    /// Metadata from the matched entry.
    pub metadata: serde_json::Value,
    /// Original content.
    pub content: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Cosine similarity score in `[-1.0, 1.0]`.
    pub score: f64,
    /// 0-based rank within the result set.
    pub rank: usize,
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

/// Cosine similarity between two equal-length vectors.
///
/// Returns `0.0` if either vector has zero magnitude or they differ in length.
#[must_use]
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// Normalize a vector to unit length. Returns a zero-vector if input has zero magnitude.
#[must_use]
pub fn normalize(v: &[f64]) -> Vec<f64> {
    let mag: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag == 0.0 {
        vec![0.0; v.len()]
    } else {
        v.iter().map(|x| x / mag).collect()
    }
}

// ---------------------------------------------------------------------------
// VectorIndex
// ---------------------------------------------------------------------------

/// In-memory vector index with brute-force cosine-similarity search.
///
/// Dimensionality is inferred from the first inserted vector or set explicitly
/// via [`VectorIndex::with_dimension`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VectorIndex {
    entries: HashMap<Uuid, VectorEntry>,
    dimension: Option<usize>,
}

impl VectorIndex {
    /// Create a new, empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            dimension: None,
        }
    }

    /// Create an index with a pre-set dimensionality.
    #[must_use]
    pub fn with_dimension(dim: usize) -> Self {
        Self {
            entries: HashMap::new(),
            dimension: Some(dim),
        }
    }

    /// Insert a vector entry. Returns its [`Uuid`].
    ///
    /// # Errors
    /// - Zero-length embedding.
    /// - Dimension mismatch with existing entries.
    pub fn insert(&mut self, entry: VectorEntry) -> Result<Uuid> {
        if entry.embedding.is_empty() {
            return Err(DaimonError::InvalidParameter(
                "cannot insert vector with zero-length embedding".into(),
            ));
        }

        match self.dimension {
            Some(dim) if dim != entry.embedding.len() => {
                return Err(DaimonError::InvalidParameter(format!(
                    "dimension mismatch: index expects {} but entry has {}",
                    dim,
                    entry.embedding.len()
                )));
            }
            None => {
                self.dimension = Some(entry.embedding.len());
            }
            _ => {}
        }

        let id = entry.id;
        debug!(id = %id, dim = entry.embedding.len(), "inserting vector entry");
        self.entries.insert(id, entry);
        Ok(id)
    }

    /// Find the `top_k` nearest neighbors to `query` by cosine similarity.
    ///
    /// Results are sorted by descending score.
    #[must_use]
    pub fn search(&self, query: &[f64], top_k: usize) -> Vec<SearchResult> {
        if top_k == 0 || self.entries.is_empty() || query.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(&VectorEntry, f64)> = self
            .entries
            .values()
            .map(|e| (e, cosine_similarity(query, &e.embedding)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        scored
            .into_iter()
            .enumerate()
            .map(|(rank, (entry, score))| SearchResult {
                id: entry.id,
                metadata: entry.metadata.clone(),
                content: entry.content.clone(),
                created_at: entry.created_at,
                score,
                rank,
            })
            .collect()
    }

    /// Remove an entry by id. Returns `true` if it existed.
    pub fn remove(&mut self, id: &Uuid) -> bool {
        self.entries.remove(id).is_some()
    }

    /// Number of entries in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Current dimensionality (if set).
    #[must_use]
    pub fn dimension(&self) -> Option<usize> {
        self.dimension
    }

    /// Persist the index to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| DaimonError::StorageError(format!("serialize vector index: {e}")))?;
        std::fs::write(path, json)
            .map_err(|e| DaimonError::StorageError(format!("write vector index: {e}")))?;
        debug!(path = %path.display(), entries = self.entries.len(), "saved vector index");
        Ok(())
    }

    /// Load an index from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| DaimonError::StorageError(format!("read vector index: {e}")))?;
        let index: Self = serde_json::from_str(&data)
            .map_err(|e| DaimonError::StorageError(format!("deserialize vector index: {e}")))?;
        debug!(path = %path.display(), entries = index.entries.len(), "loaded vector index");
        Ok(index)
    }

    /// Iterate over all entries.
    pub fn entries(&self) -> impl Iterator<Item = &VectorEntry> {
        self.entries.values()
    }
}

impl Default for VectorIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_entry(embedding: Vec<f64>, content: &str) -> VectorEntry {
        VectorEntry {
            id: Uuid::new_v4(),
            embedding,
            metadata: json!({"source": "test"}),
            content: content.to_string(),
            created_at: Utc::now(),
        }
    }

    // -- cosine_similarity --

    #[test]
    fn cosine_identical() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_orthogonal() {
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-9);
    }

    #[test]
    fn cosine_opposite() {
        assert!((cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_mismatched_lengths() {
        assert_eq!(cosine_similarity(&[1.0, 2.0], &[1.0]), 0.0);
    }

    #[test]
    fn cosine_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn cosine_zero_vector() {
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 2.0]), 0.0);
    }

    // -- normalize --

    #[test]
    fn normalize_unit() {
        let v = normalize(&[3.0, 4.0]);
        let mag: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((mag - 1.0).abs() < 1e-9);
    }

    #[test]
    fn normalize_zero() {
        assert_eq!(normalize(&[0.0, 0.0, 0.0]), vec![0.0, 0.0, 0.0]);
    }

    // -- insert --

    #[test]
    fn insert_single() {
        let mut idx = VectorIndex::new();
        let id = idx.insert(make_entry(vec![1.0, 2.0], "hello")).unwrap();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.dimension(), Some(2));
        assert!(idx.entries().any(|e| e.id == id));
    }

    #[test]
    fn insert_zero_length_rejected() {
        let mut idx = VectorIndex::new();
        assert!(idx.insert(make_entry(vec![], "bad")).is_err());
    }

    #[test]
    fn insert_dimension_mismatch() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0, 2.0], "a")).unwrap();
        assert!(idx.insert(make_entry(vec![1.0, 2.0, 3.0], "b")).is_err());
    }

    #[test]
    fn insert_same_dimension() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0, 2.0], "a")).unwrap();
        idx.insert(make_entry(vec![3.0, 4.0], "b")).unwrap();
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn with_dimension_enforced() {
        let mut idx = VectorIndex::with_dimension(3);
        assert!(idx.insert(make_entry(vec![1.0, 2.0], "bad")).is_err());
        idx.insert(make_entry(vec![1.0, 2.0, 3.0], "ok")).unwrap();
    }

    #[test]
    fn insert_duplicate_id_overwrites() {
        let mut idx = VectorIndex::new();
        let id = Uuid::new_v4();
        let e1 = VectorEntry {
            id,
            embedding: vec![1.0, 0.0],
            metadata: json!({}),
            content: "first".into(),
            created_at: Utc::now(),
        };
        let e2 = VectorEntry {
            id,
            embedding: vec![0.0, 1.0],
            metadata: json!({}),
            content: "second".into(),
            created_at: Utc::now(),
        };
        idx.insert(e1).unwrap();
        idx.insert(e2).unwrap();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.entries().next().unwrap().content, "second");
    }

    // -- search --

    #[test]
    fn search_basic() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0, 0.0], "east")).unwrap();
        idx.insert(make_entry(vec![0.0, 1.0], "north")).unwrap();

        let results = idx.search(&[1.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "east");
        assert!((results[0].score - 1.0).abs() < 1e-9);
        assert_eq!(results[0].rank, 0);
    }

    #[test]
    fn search_top_k() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0, 0.0], "a")).unwrap();
        idx.insert(make_entry(vec![0.9, 0.1], "b")).unwrap();
        idx.insert(make_entry(vec![0.0, 1.0], "c")).unwrap();

        let results = idx.search(&[1.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content, "a");
    }

    #[test]
    fn search_empty_index() {
        assert!(VectorIndex::new().search(&[1.0], 5).is_empty());
    }

    #[test]
    fn search_zero_top_k() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0], "x")).unwrap();
        assert!(idx.search(&[1.0], 0).is_empty());
    }

    #[test]
    fn search_empty_query() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0], "x")).unwrap();
        assert!(idx.search(&[], 5).is_empty());
    }

    #[test]
    fn search_ranks_descending() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0, 0.0], "a")).unwrap();
        idx.insert(make_entry(vec![0.7, 0.7], "b")).unwrap();
        idx.insert(make_entry(vec![0.0, 1.0], "c")).unwrap();

        let results = idx.search(&[1.0, 0.0], 3);
        assert_eq!(results.len(), 3);
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.rank, i);
        }
        assert!(results[0].score >= results[1].score);
        assert!(results[1].score >= results[2].score);
    }

    // -- remove --

    #[test]
    fn remove_existing() {
        let mut idx = VectorIndex::new();
        let id = idx.insert(make_entry(vec![1.0, 2.0], "bye")).unwrap();
        assert!(idx.remove(&id));
        assert!(idx.is_empty());
    }

    #[test]
    fn remove_nonexistent() {
        let mut idx = VectorIndex::new();
        assert!(!idx.remove(&Uuid::new_v4()));
    }

    // -- persistence --

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");

        let mut idx = VectorIndex::new();
        let id1 = idx
            .insert(make_entry(vec![1.0, 2.0, 3.0], "doc one"))
            .unwrap();
        idx.insert(make_entry(vec![4.0, 5.0, 6.0], "doc two"))
            .unwrap();
        idx.save(&path).unwrap();

        let loaded = VectorIndex::load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.dimension(), Some(3));
        assert!(loaded.entries().any(|e| e.id == id1));
    }

    #[test]
    fn load_nonexistent_file() {
        assert!(VectorIndex::load(Path::new("/tmp/daimon_test_missing.json")).is_err());
    }

    // -- default --

    #[test]
    fn default_is_empty() {
        let idx = VectorIndex::default();
        assert!(idx.is_empty());
        assert_eq!(idx.dimension(), None);
    }

    // -- serde roundtrip --

    #[test]
    fn vector_entry_serde_roundtrip() {
        let entry = make_entry(vec![1.0, 2.0], "hello");
        let json = serde_json::to_string(&entry).unwrap();
        let back: VectorEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, entry.id);
        assert_eq!(back.content, "hello");
    }

    #[test]
    fn search_result_serde_roundtrip() {
        let mut idx = VectorIndex::new();
        idx.insert(make_entry(vec![1.0, 0.0], "doc")).unwrap();
        let results = idx.search(&[1.0, 0.0], 1);
        let json = serde_json::to_string(&results[0]).unwrap();
        let back: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, "doc");
    }
}
