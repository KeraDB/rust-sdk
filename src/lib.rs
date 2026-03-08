//! # KeraDB Rust SDK
//!
//! A MongoDB-compatible Rust client for KeraDB — a lightweight, embedded NoSQL
//! document database with advanced vector search capabilities.
//!
//! ## Usage
//!
//! ```no_run
//! use keradb_sdk::*;
//! use serde_json::json;
//!
//! // Open / create a database
//! let mut client = connect("mydb.ndb").unwrap();
//! let db = client.database();
//! let users = db.collection("users");
//!
//! // Insert
//! let result = users.insert_one(json!({"name": "Alice", "age": 30})).unwrap();
//! println!("Inserted: {}", result.inserted_id);
//!
//! // Find
//! let doc = users.find_one(Some(&json!({"_id": result.inserted_id}))).unwrap();
//! println!("Found: {:?}", doc);
//!
//! // Update
//! users.update_one(
//!     &json!({"_id": result.inserted_id}),
//!     &json!({"$set": {"age": 31}}),
//! ).unwrap();
//!
//! // Delete
//! users.delete_one(&json!({"_id": result.inserted_id})).unwrap();
//!
//! client.close();
//! ```
//!
//! ## Vector search
//!
//! ```no_run
//! use keradb_sdk::*;
//!
//! let mut client = connect("vectors.ndb").unwrap();
//!
//! let config = VectorConfig::new(128)
//!     .with_distance(Distance::Cosine)
//!     .with_m(16)
//!     .with_ef_construction(200);
//!
//! client.create_vector_collection("embeddings", &config).unwrap();
//!
//! let embedding: Vec<f32> = (0..128).map(|i| i as f32 / 128.0).collect();
//! let id = client.insert_vector("embeddings", &embedding, None).unwrap();
//!
//! let results = client.vector_search("embeddings", &embedding, 5).unwrap();
//! for r in results {
//!     println!("{}", r);
//! }
//!
//! client.close();
//! ```

// @group Modules : Public submodule declarations

pub mod client;
pub mod error;
pub mod ffi;
pub mod results;
pub mod vector;

// @group Exports : Re-exports for ergonomic top-level usage

// Connection
pub use client::{connect, Client, Collection, Cursor, Database};

// Error
pub use error::{KeraDbError, Result};

// Result types
pub use results::{DeleteResult, InsertManyResult, InsertOneResult, UpdateResult};

// Vector types
pub use vector::{
    CompressionConfig, CompressionMode, Distance, MetadataFilter, VectorCollectionInfo,
    VectorCollectionStats, VectorConfig, VectorDocument, VectorSearchResult,
};

/// SDK version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
