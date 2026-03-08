//! Vector search example for the KeraDB Rust SDK.
//!
//! Demonstrates creating a vector collection, inserting embeddings, and
//! performing k-NN similarity search with optional metadata filtering.
//!
//! Run with:
//! ```text
//! cargo run --example vector_search
//! ```

// @group VectorSetup    : Collection creation and configuration helpers
// @group VectorInsert   : Embedding generation and insertion
// @group VectorSearch   : Basic and filtered search demonstrations
// @group VectorStats    : Collection statistics output

use keradb_sdk::{connect, Distance, MetadataFilter, VectorConfig};
use serde_json::{json, Value};
use std::fs;

// ---------------------------------------------------------------------------
// @group VectorSetup : Collection creation and configuration helpers
// ---------------------------------------------------------------------------

/// Generate a normalised random embedding of the given dimensionality.
/// Uses a deterministic pseudo-random sequence seeded from `seed`.
fn make_embedding(dimensions: usize, seed: u64) -> Vec<f32> {
    // Simple LCG pseudo-random to avoid pulling in the `rand` crate for examples
    let mut state = seed.wrapping_add(1);
    let raw: Vec<f32> = (0..dimensions)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let v = ((state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            v
        })
        .collect();

    // L2-normalise
    let norm: f32 = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        raw.into_iter().map(|x| x / norm).collect()
    } else {
        raw
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "example_vectors.ndb";
    println!("=== KeraDB Rust SDK – Vector Search Example ===\n");

    let mut client = connect(db_path)?;

    if !client.has_vector_support() {
        eprintln!(
            "This build of KeraDB does not include vector support. \
             Rebuild the native library with vector features enabled."
        );
        client.close();
        let _ = fs::remove_file(db_path);
        return Ok(());
    }

    // @group VectorSetup : Create collection
    let config = VectorConfig::new(128)
        .with_distance(Distance::Cosine)
        .with_m(16)
        .with_ef_construction(200)
        .with_ef_search(50);

    client.create_vector_collection("articles", &config)?;
    println!("Created vector collection 'articles' (128-d, cosine)\n");

    // @group VectorInsert : Insert embeddings with metadata

    println!("--- Inserting embeddings ---");

    let items: Vec<(&str, &str, u64)> = vec![
        ("Introduction to Rust",        "programming", 1),
        ("Advanced Rust Patterns",       "programming", 2),
        ("Python Machine Learning",      "ml",          3),
        ("Deep Learning Fundamentals",   "ml",          4),
        ("Database Design Principles",   "database",    5),
        ("NoSQL vs SQL Databases",       "database",    6),
        ("Rust for Systems Programming", "programming", 7),
        ("Vector Databases Explained",   "database",    8),
        ("Neural Networks in 2025",      "ml",          9),
        ("Embedded Databases in Rust",   "database",    10),
    ];

    let mut inserted_ids: Vec<u64> = Vec::new();

    for (title, category, seed) in &items {
        let embedding = make_embedding(128, *seed);
        let metadata: Value = json!({
            "title": title,
            "category": category,
            "year": 2024 + (*seed % 2) as u32,
        });
        let id = client.insert_vector("articles", &embedding, Some(&metadata))?;
        inserted_ids.push(id);
        println!("  Inserted '{}' → id={}", title, id);
    }

    // @group VectorSearch : Basic k-NN search

    println!("\n--- Basic vector search (k=5) ---");

    let query = make_embedding(128, 1); // Similar to "Introduction to Rust"
    let results = client.vector_search("articles", &query, 5)?;

    for r in &results {
        let title = r.document.metadata.get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("?");
        println!("  rank={} score={:.4}  id={}  '{}'", r.rank, r.score, r.document.id, title);
    }

    // @group VectorSearch : Filtered search

    println!("\n--- Filtered search: category = 'database' (k=5) ---");

    let filter = MetadataFilter::eq("category", json!("database"));
    let filtered = client.vector_search_filtered("articles", &query, 5, &filter)?;

    for r in &filtered {
        let title = r.document.metadata.get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("?");
        let cat = r.document.metadata.get("category")
            .and_then(|c| c.as_str())
            .unwrap_or("?");
        println!("  rank={} score={:.4}  category={}  '{}'", r.rank, r.score, cat, title);
    }

    // @group VectorSearch : Retrieve a vector by ID

    println!("\n--- Get vector by ID ---");
    if let Some(first_id) = inserted_ids.first() {
        if let Some(doc) = client.get_vector("articles", *first_id)? {
            let title = doc.metadata.get("title").and_then(|t| t.as_str()).unwrap_or("?");
            println!("  get_vector(id={}) → title='{}'", doc.id, title);
        }
    }

    // @group VectorSearch : Delete a vector

    println!("\n--- Delete a vector ---");
    if let Some(last_id) = inserted_ids.last() {
        let deleted = client.delete_vector("articles", *last_id)?;
        println!("  delete_vector(id={}) → {}", last_id, deleted);
    }

    // @group VectorStats : Collection statistics

    println!("\n--- Collection statistics ---");
    let stats = client.vector_stats("articles")?;
    println!("  vectors     : {}", stats.vector_count);
    println!("  dimensions  : {}", stats.dimensions);
    println!("  distance    : {}", stats.distance);
    println!("  memory      : {} bytes", stats.memory_usage);
    println!("  layers      : {}", stats.layer_count);
    println!("  lazy embed  : {}", stats.lazy_embedding);
    if let Some(mode) = &stats.compression {
        println!("  compression : {}", mode);
    }

    // List collections
    println!("\n--- All vector collections ---");
    let collections = client.list_vector_collections()?;
    for c in &collections {
        println!("  {} ({} vectors)", c.name, c.count);
    }

    client.close();
    println!("\nDone ✓");

    let _ = fs::remove_file(db_path);
    Ok(())
}
