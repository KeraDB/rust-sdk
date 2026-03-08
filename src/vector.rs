//! Vector search types mirroring the Python SDK's `vector.py`.

// @group VectorConfig     : Configuration types for vector collections
// @group VectorDocuments  : Document and search result types
// @group VectorFilter     : Metadata filter for filtered search

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// @group VectorConfig : Configuration types for vector collections
// ---------------------------------------------------------------------------

/// Distance metric used for vector similarity comparisons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distance {
    /// Cosine similarity – range \[0, 2\], 0 = identical.
    Cosine,
    /// Euclidean (L2) distance.
    Euclidean,
    /// Negative dot product for similarity ranking.
    DotProduct,
    /// Manhattan (L1) distance.
    Manhattan,
}

impl Distance {
    /// Return the wire string used in JSON configuration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Distance::Cosine => "cosine",
            Distance::Euclidean => "euclidean",
            Distance::DotProduct => "dot_product",
            Distance::Manhattan => "manhattan",
        }
    }
}

impl std::fmt::Display for Distance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Vector storage compression mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    /// No compression – full precision vectors.
    None,
    /// Delta compression – sparse differences from neighbours.
    Delta,
    /// Aggressive quantised deltas.
    QuantizedDelta,
}

impl CompressionMode {
    /// Return the wire string.
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionMode::None => "none",
            CompressionMode::Delta => "delta",
            CompressionMode::QuantizedDelta => "quantized_delta",
        }
    }
}

impl std::fmt::Display for CompressionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Fine-grained configuration for vector compression.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CompressionConfig {
    /// Compression mode.
    pub mode: Option<CompressionMode>,
    /// Threshold for considering a vector sparse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparsity_threshold: Option<f64>,
    /// Maximum allowed density for delta compression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_density: Option<f64>,
    /// Frequency (in insertions) at which anchor vectors are written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_frequency: Option<u32>,
    /// Bit-width used during quantisation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization_bits: Option<u32>,
}

impl CompressionConfig {
    /// Create a config with a specific mode and no extra parameters.
    pub fn new(mode: CompressionMode) -> Self {
        Self {
            mode: Some(mode),
            ..Default::default()
        }
    }

    /// Delta compression with default parameters.
    pub fn delta() -> Self {
        Self::new(CompressionMode::Delta)
    }

    /// Quantised-delta compression with default parameters.
    pub fn quantized_delta() -> Self {
        Self::new(CompressionMode::QuantizedDelta)
    }

    /// Serialise to a `serde_json::Value` map.
    pub fn to_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        if let Some(mode) = &self.mode {
            map.insert("mode".into(), Value::String(mode.as_str().to_owned()));
        }
        if let Some(v) = self.sparsity_threshold {
            map.insert("sparsity_threshold".into(), Value::from(v));
        }
        if let Some(v) = self.max_density {
            map.insert("max_density".into(), Value::from(v));
        }
        if let Some(v) = self.anchor_frequency {
            map.insert("anchor_frequency".into(), Value::from(v));
        }
        if let Some(v) = self.quantization_bits {
            map.insert("quantization_bits".into(), Value::from(v));
        }
        Value::Object(map)
    }
}

/// Configuration for a vector collection, using a builder-style API that
/// mirrors `VectorConfig` in the Python SDK.
#[derive(Debug, Clone)]
pub struct VectorConfig {
    /// Number of dimensions in each embedding.
    pub dimensions: usize,
    /// Distance metric (default: [`Distance::Cosine`]).
    pub distance: Distance,
    /// HNSW `M` parameter – connections per node (default: library default).
    pub m: Option<u32>,
    /// HNSW `ef_construction` – build quality (default: library default).
    pub ef_construction: Option<u32>,
    /// HNSW `ef_search` – query quality (default: library default).
    pub ef_search: Option<u32>,
    /// Enable lazy embedding mode (store text, embed on demand).
    pub lazy_embedding: bool,
    /// Model name to use for lazy embedding.
    pub embedding_model: Option<String>,
    /// Optional vector compression settings.
    pub compression: Option<CompressionConfig>,
}

impl VectorConfig {
    /// Create a minimal config with the given number of dimensions.
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            distance: Distance::Cosine,
            m: None,
            ef_construction: None,
            ef_search: None,
            lazy_embedding: false,
            embedding_model: None,
            compression: None,
        }
    }

    // Builder methods

    /// Set the distance metric.
    pub fn with_distance(mut self, distance: Distance) -> Self {
        self.distance = distance;
        self
    }

    /// Set the HNSW `M` parameter.
    pub fn with_m(mut self, m: u32) -> Self {
        self.m = Some(m);
        self
    }

    /// Set the `ef_construction` parameter.
    pub fn with_ef_construction(mut self, ef: u32) -> Self {
        self.ef_construction = Some(ef);
        self
    }

    /// Set the `ef_search` parameter.
    pub fn with_ef_search(mut self, ef: u32) -> Self {
        self.ef_search = Some(ef);
        self
    }

    /// Enable lazy embedding using an embedding model name.
    pub fn with_lazy_embedding(mut self, model: impl Into<String>) -> Self {
        self.lazy_embedding = true;
        self.embedding_model = Some(model.into());
        self
    }

    /// Set a custom compression configuration.
    pub fn with_compression(mut self, config: CompressionConfig) -> Self {
        self.compression = Some(config);
        self
    }

    /// Enable delta compression with default settings.
    pub fn with_delta_compression(self) -> Self {
        self.with_compression(CompressionConfig::delta())
    }

    /// Enable quantised-delta compression with default settings.
    pub fn with_quantized_compression(self) -> Self {
        self.with_compression(CompressionConfig::quantized_delta())
    }

    /// Serialise to JSON for passing to the native C API.
    pub fn to_json(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("dimensions".into(), Value::from(self.dimensions));
        map.insert(
            "distance".into(),
            Value::String(self.distance.as_str().to_owned()),
        );
        if let Some(m) = self.m {
            map.insert("m".into(), Value::from(m));
        }
        if let Some(ef) = self.ef_construction {
            map.insert("ef_construction".into(), Value::from(ef));
        }
        if let Some(ef) = self.ef_search {
            map.insert("ef_search".into(), Value::from(ef));
        }
        if self.lazy_embedding {
            map.insert("lazy_embedding".into(), Value::Bool(true));
            if let Some(model) = &self.embedding_model {
                map.insert("embedding_model".into(), Value::String(model.clone()));
            }
        }
        if let Some(comp) = &self.compression {
            map.insert("compression".into(), comp.to_value());
        }
        Value::Object(map).to_string()
    }
}

// ---------------------------------------------------------------------------
// @group VectorDocuments : Document and search result types
// ---------------------------------------------------------------------------

/// A document stored in a vector collection.
#[derive(Debug, Clone)]
pub struct VectorDocument {
    /// Numeric ID assigned by KeraDB.
    pub id: u64,
    /// The stored embedding (may be absent if not requested).
    pub embedding: Option<Vec<f32>>,
    /// Original text if stored via lazy embedding.
    pub text: Option<String>,
    /// User-supplied metadata as arbitrary JSON.
    pub metadata: Value,
}

impl VectorDocument {
    /// Deserialise from the JSON object returned by the native library.
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = v.get("id")?.as_u64()?;
        let embedding = v.get("embedding").and_then(|e| {
            e.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_f64().map(|f| f as f32))
                    .collect()
            })
        });
        let text = v.get("text").and_then(|t| t.as_str()).map(String::from);
        let metadata = v
            .get("metadata")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        Some(Self {
            id,
            embedding,
            text,
            metadata,
        })
    }
}

impl std::fmt::Display for VectorDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VectorDocument(id={}, metadata={})", self.id, self.metadata)
    }
}

/// A single result from a vector similarity search.
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    /// The matched document.
    pub document: VectorDocument,
    /// Similarity or distance score (lower is closer for distance metrics).
    pub score: f64,
    /// 1-based rank in the result set.
    pub rank: usize,
}

impl VectorSearchResult {
    /// Deserialise from the JSON object in the search results array.
    pub fn from_value(v: &Value) -> Option<Self> {
        let doc = VectorDocument::from_value(v.get("document")?)?;
        let score = v.get("score")?.as_f64()?;
        let rank = v.get("rank")?.as_u64()? as usize;
        Some(Self {
            document: doc,
            score,
            rank,
        })
    }
}

impl std::fmt::Display for VectorSearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VectorSearchResult(rank={}, score={:.4}, id={})",
            self.rank, self.score, self.document.id
        )
    }
}

/// Runtime statistics for a vector collection.
#[derive(Debug, Clone)]
pub struct VectorCollectionStats {
    /// Total number of vectors stored.
    pub vector_count: usize,
    /// Per-vector dimensionality.
    pub dimensions: usize,
    /// Distance metric in use.
    pub distance: Distance,
    /// Approximate memory usage in bytes.
    pub memory_usage: usize,
    /// Number of HNSW layers in the index.
    pub layer_count: usize,
    /// Whether lazy embedding is enabled.
    pub lazy_embedding: bool,
    /// Active compression mode (if any).
    pub compression: Option<CompressionMode>,
    /// Number of anchor vectors (delta/quantised modes only).
    pub anchor_count: Option<usize>,
    /// Number of delta vectors (delta/quantised modes only).
    pub delta_count: Option<usize>,
}

impl VectorCollectionStats {
    /// Deserialise from the JSON object returned by `keradb_vector_stats`.
    pub fn from_value(v: &Value) -> Option<Self> {
        let distance_str = v.get("distance")?.as_str()?;
        let distance = match distance_str {
            "cosine" => Distance::Cosine,
            "euclidean" => Distance::Euclidean,
            "dot_product" => Distance::DotProduct,
            "manhattan" => Distance::Manhattan,
            _ => Distance::Cosine,
        };
        let compression = v.get("compression").and_then(|c| c.as_str()).map(|s| {
            match s {
                "delta" => CompressionMode::Delta,
                "quantized_delta" => CompressionMode::QuantizedDelta,
                _ => CompressionMode::None,
            }
        });

        Some(Self {
            vector_count: v.get("vector_count")?.as_u64()? as usize,
            dimensions: v.get("dimensions")?.as_u64()? as usize,
            distance,
            memory_usage: v.get("memory_usage").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
            layer_count: v.get("layer_count").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
            lazy_embedding: v
                .get("lazy_embedding")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            compression,
            anchor_count: v
                .get("anchor_count")
                .and_then(|x| x.as_u64())
                .map(|x| x as usize),
            delta_count: v
                .get("delta_count")
                .and_then(|x| x.as_u64())
                .map(|x| x as usize),
        })
    }
}

impl std::fmt::Display for VectorCollectionStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VectorCollectionStats(vectors={}, dimensions={}, distance={}, memory={} bytes)",
            self.vector_count, self.dimensions, self.distance, self.memory_usage
        )
    }
}

// ---------------------------------------------------------------------------
// @group VectorFilter : Metadata filter for filtered vector search
// ---------------------------------------------------------------------------

/// A filter on a metadata field, used in [`Client::vector_search_filtered`].
///
/// Supported conditions match the Python SDK:
/// `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`, `contains`,
/// `starts_with`, `ends_with`.
#[derive(Debug, Clone, Serialize)]
pub struct MetadataFilter {
    /// The metadata field name to filter on.
    pub field: String,
    /// The condition type (e.g. `"eq"`, `"gt"`, `"in"`).
    pub condition: String,
    /// The comparison value as a JSON value.
    pub value: Value,
}

impl MetadataFilter {
    /// Create a new metadata filter.
    pub fn new(field: impl Into<String>, condition: impl Into<String>, value: Value) -> Self {
        Self {
            field: field.into(),
            condition: condition.into(),
            value,
        }
    }

    /// Equality filter shorthand.
    pub fn eq(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, "eq", value)
    }

    /// Greater-than filter shorthand.
    pub fn gt(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, "gt", value)
    }

    /// Less-than filter shorthand.
    pub fn lt(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, "lt", value)
    }

    /// Serialise to a JSON string for passing to the C API.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }
}

/// Slim info struct returned by `Client::list_vector_collections`.
#[derive(Debug, Clone)]
pub struct VectorCollectionInfo {
    /// Collection name.
    pub name: String,
    /// Number of vectors in the collection.
    pub count: usize,
}

// ---------------------------------------------------------------------------
// @group UnitTests : Vector type construction, serialisation and parsing
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Distance ---

    #[test]
    fn distance_as_str() {
        assert_eq!(Distance::Cosine.as_str(), "cosine");
        assert_eq!(Distance::Euclidean.as_str(), "euclidean");
        assert_eq!(Distance::DotProduct.as_str(), "dot_product");
        assert_eq!(Distance::Manhattan.as_str(), "manhattan");
    }

    #[test]
    fn distance_display() {
        assert_eq!(Distance::Cosine.to_string(), "cosine");
    }

    // --- CompressionMode ---

    #[test]
    fn compression_mode_as_str() {
        assert_eq!(CompressionMode::None.as_str(), "none");
        assert_eq!(CompressionMode::Delta.as_str(), "delta");
        assert_eq!(CompressionMode::QuantizedDelta.as_str(), "quantized_delta");
    }

    // --- VectorConfig builder ---

    #[test]
    fn vector_config_defaults() {
        let cfg = VectorConfig::new(128);
        assert_eq!(cfg.dimensions, 128);
        assert_eq!(cfg.distance, Distance::Cosine);
        assert_eq!(cfg.m, None);
        assert_eq!(cfg.ef_construction, None);
        assert_eq!(cfg.ef_search, None);
        assert!(!cfg.lazy_embedding);
    }

    #[test]
    fn vector_config_builder_chain() {
        let cfg = VectorConfig::new(256)
            .with_distance(Distance::Euclidean)
            .with_m(32)
            .with_ef_construction(400)
            .with_ef_search(80);
        assert_eq!(cfg.dimensions, 256);
        assert_eq!(cfg.distance, Distance::Euclidean);
        assert_eq!(cfg.m, Some(32));
        assert_eq!(cfg.ef_construction, Some(400));
        assert_eq!(cfg.ef_search, Some(80));
    }

    #[test]
    fn vector_config_lazy_embedding() {
        let cfg = VectorConfig::new(768).with_lazy_embedding("my-model");
        assert!(cfg.lazy_embedding);
        assert_eq!(cfg.embedding_model.as_deref(), Some("my-model"));
    }

    #[test]
    fn vector_config_delta_compression() {
        let cfg = VectorConfig::new(64).with_delta_compression();
        assert!(cfg.compression.is_some());
        let c = cfg.compression.as_ref().unwrap();
        assert_eq!(c.mode, Some(CompressionMode::Delta));
    }

    #[test]
    fn vector_config_to_json_roundtrip() {
        let cfg = VectorConfig::new(128)
            .with_distance(Distance::DotProduct)
            .with_m(24);
        let s = cfg.to_json();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["dimensions"], json!(128));
        assert_eq!(v["distance"], json!("dot_product"));
        assert_eq!(v["m"], json!(24));
    }

    // --- MetadataFilter ---

    #[test]
    fn metadata_filter_eq_shorthand() {
        let f = MetadataFilter::eq("category", json!("news"));
        assert_eq!(f.field, "category");
        assert_eq!(f.condition, "eq");
        assert_eq!(f.value, json!("news"));
    }

    #[test]
    fn metadata_filter_gt_shorthand() {
        let f = MetadataFilter::gt("score", json!(0.8));
        assert_eq!(f.condition, "gt");
    }

    #[test]
    fn metadata_filter_lt_shorthand() {
        let f = MetadataFilter::lt("score", json!(0.5));
        assert_eq!(f.condition, "lt");
    }

    #[test]
    fn metadata_filter_to_json() {
        let f = MetadataFilter::eq("lang", json!("en"));
        let s = f.to_json();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["field"], json!("lang"));
        assert_eq!(v["condition"], json!("eq"));
        assert_eq!(v["value"], json!("en"));
    }

    // --- VectorSearchResult ---

    #[test]
    fn vector_search_result_from_value() {
        let v = json!({
            "document": {"id": 7, "metadata": {"label": "A"}},
            "score": 0.95,
            "rank": 1
        });
        let r = VectorSearchResult::from_value(&v).unwrap();
        assert_eq!(r.document.id, 7);
        assert!((r.score - 0.95).abs() < 1e-6);
        assert_eq!(r.rank, 1);
        assert_eq!(r.document.metadata["label"], json!("A"));
    }

    #[test]
    fn vector_search_result_display() {
        let v = json!({
            "document": {"id": 1, "metadata": {}},
            "score": 0.5,
            "rank": 1
        });
        let r = VectorSearchResult::from_value(&v).unwrap();
        let s = r.to_string();
        assert!(s.contains("id=1"));
        assert!(s.contains("score="));
    }

    #[test]
    fn vector_search_result_missing_document_returns_none() {
        let v = json!({"score": 0.9, "rank": 1});
        assert!(VectorSearchResult::from_value(&v).is_none());
    }

    // --- VectorDocument ---

    #[test]
    fn vector_document_from_value() {
        let v = json!({"id": 3, "embedding": [0.1, 0.2, 0.3], "metadata": {"tag": "x"}});
        let d = VectorDocument::from_value(&v).unwrap();
        assert_eq!(d.id, 3);
        let emb = d.embedding.unwrap();
        assert!((emb[0] - 0.1f32).abs() < 1e-6);
        assert_eq!(d.metadata["tag"], json!("x"));
    }

    #[test]
    fn vector_document_missing_id_returns_none() {
        let v = json!({"embedding": [0.1], "metadata": {}});
        assert!(VectorDocument::from_value(&v).is_none());
    }

    #[test]
    fn vector_document_optional_embedding() {
        let v = json!({"id": 5, "metadata": {}});
        let d = VectorDocument::from_value(&v).unwrap();
        assert!(d.embedding.is_none());
    }

    // --- CompressionConfig ---

    #[test]
    fn compression_config_delta() {
        let c = CompressionConfig::delta();
        assert_eq!(c.mode, Some(CompressionMode::Delta));
        assert!(c.quantization_bits.is_none());
    }

    #[test]
    fn compression_config_quantized_delta() {
        let c = CompressionConfig::quantized_delta();
        assert_eq!(c.mode, Some(CompressionMode::QuantizedDelta));
    }

    #[test]
    fn compression_config_with_quantization_bits() {
        let c = CompressionConfig {
            mode: Some(CompressionMode::QuantizedDelta),
            quantization_bits: Some(8),
            ..Default::default()
        };
        assert_eq!(c.mode, Some(CompressionMode::QuantizedDelta));
        assert_eq!(c.quantization_bits, Some(8));
    }
}
