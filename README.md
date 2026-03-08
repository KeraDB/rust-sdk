# KeraDB Rust SDK

The native Rust SDK for KeraDB - a lightweight, embedded NoSQL document database with vector search capabilities.

## Features

- **Document Storage**: JSON document storage with collections
- **Vector Database**: HNSW-based similarity search with multiple distance metrics
- **LEANN-Style Compression**: Up to 97% storage savings for vectors
- **Lazy Embeddings**: Store text, compute embeddings on-demand
- **Cross-Platform**: Windows, macOS, Linux support

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
keradb = "0.1"
serde_json = "1.0"
```

For development from source:

```toml
[dependencies]
keradb = { path = "../.." }
serde_json = "1.0"
```

## Quick Start

### Document Database

```rust
use keradb::Database;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create or open database
    let db = Database::create("mydata.ndb")?;
    
    // Insert a document
    let id = db.insert("users", json!({
        "name": "Alice",
        "age": 30,
        "email": "alice@example.com"
    }))?;
    
    println!("Inserted document with ID: {}", id);
    
    // Find by ID
    let doc = db.find_by_id("users", &id)?;
    println!("Found: {:?}", doc);
    
    // Update
    db.update("users", &id, json!({
        "name": "Alice",
        "age": 31,
        "email": "alice@example.com"
    }))?;
    
    // Find all documents
    let all_docs = db.find_all("users", None, None)?;
    println!("Total documents: {}", all_docs.len());
    
    // Count
    let count = db.count("users");
    println!("Count: {}", count);
    
    // List collections
    let collections = db.list_collections();
    for (name, count) in collections {
        println!("Collection '{}' has {} documents", name, count);
    }
    
    // Delete
    db.delete("users", &id)?;
    
    // Sync to disk
    db.sync()?;
    
    Ok(())
}
```

### With Custom Configuration

```rust
use keradb::{Database, Config};

let config = Config {
    page_size: 8192,
    cache_size: 1000,
    ..Default::default()
};

let db = Database::create_with_config("mydata.ndb", config)?;
```

### Pagination

```rust
// Get first 10 documents
let page1 = db.find_all("users", Some(10), None)?;

// Get next 10 documents (skip first 10)
let page2 = db.find_all("users", Some(10), Some(10))?;
```

### Error Handling

```rust
use keradb::{Database, KeraDBError};

match db.find_by_id("users", "non-existent-id") {
    Ok(doc) => println!("Found: {:?}", doc),
    Err(KeraDBError::DocumentNotFound(id)) => {
        println!("Document {} not found", id);
    }
    Err(e) => println!("Error: {}", e),
}
```

## Vector Database

KeraDB includes powerful vector database capabilities for AI/ML applications, semantic search, and similarity queries.

### Creating a Vector Collection

```rust
use keradb::{Database, VectorConfig, Distance};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::create("vectors.ndb")?;
    
    // Create a vector collection with configuration
    let config = VectorConfig::new(384)  // 384 dimensions (e.g., all-MiniLM-L6-v2)
        .with_distance(Distance::Cosine)
        .with_m(16)                       // HNSW M parameter
        .with_ef_construction(200);       // HNSW ef_construction
    
    db.create_vector_collection("embeddings", config)?;
    
    Ok(())
}
```

### Inserting Vectors

```rust
use keradb::{Database, VectorConfig, Distance};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::create("vectors.ndb")?;
    
    let config = VectorConfig::new(4).with_distance(Distance::Cosine);
    db.create_vector_collection("embeddings", config)?;
    
    // Insert vectors with metadata
    let vectors = vec![
        (vec![1.0, 0.0, 0.0, 0.0], json!({"label": "north", "category": "direction"})),
        (vec![0.0, 1.0, 0.0, 0.0], json!({"label": "east", "category": "direction"})),
        (vec![0.7, 0.7, 0.0, 0.0], json!({"label": "northeast", "category": "direction"})),
        (vec![0.5, 0.5, 0.5, 0.5], json!({"label": "center", "category": "special"})),
    ];
    
    for (vector, metadata) in vectors {
        let id = db.insert_vector("embeddings", vector, Some(metadata))?;
        println!("Inserted vector with ID: {}", id);
    }
    
    Ok(())
}
```

### Vector Similarity Search

```rust
use keradb::{Database, VectorConfig, Distance};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::create("vectors.ndb")?;
    
    let config = VectorConfig::new(4).with_distance(Distance::Cosine);
    db.create_vector_collection("embeddings", config)?;
    
    // Insert some vectors
    db.insert_vector("embeddings", vec![1.0, 0.0, 0.0, 0.0], 
                     Some(json!({"label": "north"})))?;
    db.insert_vector("embeddings", vec![0.7, 0.7, 0.0, 0.0], 
                     Some(json!({"label": "northeast"})))?;
    db.insert_vector("embeddings", vec![0.0, 1.0, 0.0, 0.0], 
                     Some(json!({"label": "east"})))?;
    
    // Search for similar vectors (k nearest neighbors)
    let query = vec![0.8, 0.6, 0.0, 0.0];
    let results = db.vector_search("embeddings", &query, 3)?;
    
    println!("Top 3 similar vectors:");
    for result in results {
        println!("  • {} (score: {:.4})", 
                 result.document.metadata["label"],
                 result.score);
    }
    
    Ok(())
}
```

### Distance Metrics

KeraDB supports multiple distance metrics:

```rust
use keradb::{VectorConfig, Distance};

// Cosine similarity (default) - best for normalized embeddings
let config = VectorConfig::new(384).with_distance(Distance::Cosine);

// Euclidean (L2) distance - best for spatial data
let config = VectorConfig::new(384).with_distance(Distance::Euclidean);

// Dot product - best for unnormalized embeddings
let config = VectorConfig::new(384).with_distance(Distance::DotProduct);

// Manhattan (L1) distance
let config = VectorConfig::new(384).with_distance(Distance::Manhattan);
```

### LEANN-Style Compression (97% Storage Savings)

Enable delta or quantized compression for massive storage savings:

```rust
use keradb::{Database, VectorConfig, Distance, CompressionConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::create("compressed.ndb")?;
    
    // Enable delta compression (up to 97% storage savings)
    let config = VectorConfig::new(384)
        .with_distance(Distance::Cosine)
        .with_delta_compression();
    
    db.create_vector_collection("embeddings", config)?;
    
    // Or use quantized compression
    let quantized_config = VectorConfig::new(384)
        .with_quantized_compression();
    
    db.create_vector_collection("quantized_embeddings", quantized_config)?;
    
    Ok(())
}
```

### Lazy Embeddings (Text-to-Vector)

Store text and compute embeddings on-demand:

```rust
use keradb::{Database, VectorConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::create("lazy.ndb")?;
    
    // Enable lazy embedding mode
    let config = VectorConfig::new(384)
        .with_lazy_embedding("all-MiniLM-L6-v2");
    
    db.create_vector_collection("documents", config)?;
    
    Ok(())
}
```

### Vector Collection Statistics

```rust
use keradb::{Database, VectorConfig, Distance};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::create("vectors.ndb")?;
    
    let config = VectorConfig::new(384).with_distance(Distance::Cosine);
    db.create_vector_collection("embeddings", config)?;
    
    // Get collection statistics
    let stats = db.vector_stats("embeddings")?;
    
    println!("Vector Collection Stats:");
    println!("  Vectors: {}", stats.vector_count);
    println!("  Dimensions: {}", stats.dimensions);
    println!("  Distance Metric: {}", stats.distance.name());
    
    Ok(())
}
```

## API Reference

### Database

#### Creation and Opening

| Method | Description |
|--------|-------------|
| `Database::create(path)` | Create a new database |
| `Database::create_with_config(path, config)` | Create with custom config |
| `Database::open(path)` | Open an existing database |
| `Database::open_with_config(path, config)` | Open with custom config |

#### Document Operations

| Method | Description |
|--------|-------------|
| `insert(collection, data)` | Insert a document, returns document ID |
| `find_by_id(collection, doc_id)` | Find a document by ID |
| `update(collection, doc_id, data)` | Update a document |
| `delete(collection, doc_id)` | Delete a document |
| `find_all(collection, limit, skip)` | Find all documents with pagination |

#### Collection Operations

| Method | Description |
|--------|-------------|
| `count(collection)` | Count documents in a collection |
| `list_collections()` | List all collections with document counts |
| `sync()` | Flush all changes to disk |

#### Vector Operations

| Method | Description |
|--------|-------------|
| `create_vector_collection(name, config)` | Create a vector-enabled collection |
| `insert_vector(collection, vector, metadata)` | Insert a vector with optional metadata |
| `vector_search(collection, query, k)` | Search for k nearest neighbors |
| `vector_stats(collection)` | Get vector collection statistics |

### Types

| Type | Description |
|------|-------------|
| `Document` | A document with an ID and JSON data |
| `DocumentId` | String type for document IDs (UUIDs) |
| `Config` | Database configuration |
| `VectorConfig` | Configuration for vector collections |
| `VectorDocument` | A vector with ID, embedding, and metadata |
| `VectorSearchResult` | Search result with document and score |
| `Distance` | Distance metric enum (Cosine, Euclidean, DotProduct, Manhattan) |
| `CompressionConfig` | Compression settings for vectors |
| `KeraDBError` | Error type for database operations |

### VectorConfig Builder

```rust
VectorConfig::new(dimensions)
    .with_distance(Distance::Cosine)     // Distance metric
    .with_m(16)                          // HNSW M parameter
    .with_ef_construction(200)           // HNSW build quality
    .with_delta_compression()            // Enable LEANN compression
    .with_lazy_embedding("model-name")   // Enable lazy embeddings
```

## Building the C Library

To build the FFI-compatible library for use with other languages:

```bash
# Build dynamic library
cargo build --release

# The library will be at:
# - Linux: target/release/libkeradb.so
# - macOS: target/release/libkeradb.dylib
# - Windows: target/release/keradb.dll

# Static library: target/release/libkeradb.a
```

## Examples

See the `examples/` directory for more examples:

```bash
cargo run --example basic
cargo run --example vector_search
```

## Testing

The SDK has **67 tests** across three categories:

### Unit tests (co-located in `src/`)

These test pure Rust logic with no native library required — always runnable.

| Suite | Count | What it tests |
|---|---|---|
| `error::tests` | 9 | `KeraDbError` variant messages, `From<serde_json::Error>` conversion, `Result<T>` alias |
| `results::tests` | 8 | `InsertOneResult`, `InsertManyResult`, `UpdateResult`, `DeleteResult` — construction, field values, `Display` formatting |
| `vector::tests` | 22 | `Distance` / `CompressionMode` string values; `VectorConfig` builder chain and JSON serialisation; `MetadataFilter` shorthands (`eq`, `gt`, `lt`) and JSON output; `VectorDocument` and `VectorSearchResult` deserialisation from native JSON shape |

### Integration tests (`tests/integration_test.rs`)

Logic tests that also run without the native library:

| Group | Count | What it tests |
|---|---|---|
| Filter matching | 4 | `matches_filter()` — direct equality, `$gt`/`$gte`/`$lt`/`$lte`/`$ne`, `$in`/`$nin`, `$and`/`$or` |
| Update operators | 5 | `apply_update()` — `$set`, `$unset`, `$inc`, `$push`, full document replacement (with `_id` preservation) |
| Cursor | 4 | `limit()`, `skip()`, combined `limit + skip`, `IntoIterator` |
| VectorConfig JSON | 3 | `to_json()` roundtrip, delta compression flag, lazy embedding fields |

CRUD tests that require the native `keradb` shared library (7 tests):

```bash
cargo test --features integration
```

Covers `insert_one`, `find_one` by ID, `find` (all), `update_one`, `delete_one`, `count_documents`, `insert_many`.

### Doc-tests

5 compile-checked code examples embedded in the source docs (`lib.rs`, `client.rs`).

### Run all non-native tests

```bash
cargo test
```

```text
test result: ok. 39 passed; 0 failed  (lib unit tests)
test result: ok. 16 passed; 0 failed  (integration logic tests)
test result: ok.  5 passed; 0 failed  (doc-tests)
```

## Benchmarks

Criterion-based benchmarks live in `benches/`. Two suites are included:

| File | What it measures |
|---|---|
| `bench_documents.rs` | Document CRUD — KeraDB vs SQLite in-memory |
| `bench_vectors.rs` | Vector k-NN — KeraDB HNSW vs brute-force linear scan |

### Running

SQLite baseline + brute-force linear scan (no native library required):

```bash
cargo bench
```

Full KeraDB vs SQLite + HNSW vs brute-force comparison (requires native `keradb` shared library):

```bash
cargo bench --features integration
```

HTML reports are written to `target/criterion/`.

### Results (March 2026, Windows, AMD Ryzen)

Run with `cargo bench --features integration`. Three modes compared:

- **SQLite/inmem** — `Connection::open_in_memory()`, no fsync (best case for SQLite)
- **SQLite/ondisk** — WAL + `PRAGMA synchronous=NORMAL`, temp file (fair comparison with KeraDB)
- **KeraDB** — temp file on disk (default mode)

#### Document operations

| Benchmark | SQLite/inmem | SQLite/ondisk | KeraDB | KeraDB vs ondisk |
|---|---|---|---|---|
| `insert_one` | 22 µs | — | 106 µs | SQLite ~5× faster |
| `insert_batch` (100 docs) | 8.7 ms · 115 K/s | 26.9 ms · 37 K/s | 90 ms · 11 K/s | SQLite ~3× faster |
| `find_by_id` | 1.4 µs | — | 7.1 µs | SQLite ~5× faster |
| `find_all` (100 docs) | 59 µs | 68 µs | 722 µs | SQLite ~11× faster |
| `update_one` | 5.2 µs | — | 70.8 µs | SQLite ~14× faster |
| `delete_one` | 20.6 µs | **185 µs** | **127 µs** | **KeraDB 1.5× faster** |
| `count_documents` | 3.1 µs | 10.2 µs | **236 ns** | **KeraDB 43× faster** |
| `bulk_throughput` (1 000 docs) | 8.7 ms · 115 K/s | 26.9 ms · 37 K/s | 90 ms · 11 K/s | SQLite ~3× faster |

#### Key findings

- **`count_documents`**: KeraDB stores count as a hot in-memory integer — **43× faster** than SQLite on-disk, 13× faster than SQLite in-memory.
- **`delete_one`**: KeraDB beats SQLite on-disk (127 µs vs 185 µs). WAL flush on delete is SQLite's bottleneck here.
- **Write throughput gap** is mostly an in-memory vs on-disk artefact. Against SQLite on-disk with WAL, the gap narrows from 8× to ~3×.
- **`find_by_id`** was previously reported as equal — a benchmark bug (SQLite was re-preparing the statement every iteration, inflating its time). Fixed result: SQLite inmem is ~5× faster for single-key reads.
- **`insert_batch`** now uses `BEGIN`/`COMMIT` for SQLite, which is 1.7× faster than previous autocommit-per-row, giving a more realistic comparison.

#### Vector search — brute-force cosine scan (128-dim baseline)

| Benchmark | Corpus | Time | Throughput |
|---|---|---|---|
| `linear_scan` | 500 vecs | 81.7 µs | 6.1 M elem/s |
| `linear_scan` | 5 000 vecs | 987 µs | 5.1 M elem/s |

> KeraDB HNSW vector benchmarks require a vector-enabled native build (`keradb_create_vector_collection` symbol). HNSW is O(log N) vs O(N) for linear scan — at 5 K+ vectors the gap grows into orders of magnitude.

## License

MIT License
