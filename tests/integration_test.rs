//! Integration tests for the KeraDB Rust SDK.
//!
//! These tests require a built `keradb` shared library in the standard
//! `target/release/` path.  Run with:
//!
//! ```text
//! cargo test
//! ```

// @group TestSetup    : Test fixtures and helpers
// @group UnitTests    : Core document CRUD tests
// @group CursorTests  : Cursor limit / skip behaviour
// @group FilterTests  : MongoDB-style filter matching
// @group UpdateTests  : Apply-update operator logic
// @group VectorTests  : Vector collection and search tests

use keradb_sdk::{client::{apply_update, matches_filter}, Distance, VectorConfig};
use serde_json::json;

#[cfg(feature = "integration")]
use keradb_sdk::connect;
#[cfg(feature = "integration")]
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// @group TestSetup : Test fixtures and helpers
// ---------------------------------------------------------------------------

/// Create an anonymous temp file and open a Client on it.
/// Returns `(client, _guard)` – the guard must stay alive for the file to exist.
#[cfg(feature = "integration")]
macro_rules! temp_client {
    () => {{
        let tmp = NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_str().unwrap().to_owned();
        // Remove the file so KeraDB can create its own
        drop(tmp);
        let client = connect(&(path.clone() + ".ndb")).expect("connect");
        (client, path)
    }};
}

// ---------------------------------------------------------------------------
// @group FilterTests : MongoDB-style filter matching (no library needed)
// ---------------------------------------------------------------------------

#[test]
fn test_matches_filter_direct_equality() {
    let doc = json!({"name": "Alice", "age": 30});
    assert!(matches_filter(&doc, &json!({"name": "Alice"})));
    assert!(!matches_filter(&doc, &json!({"name": "Bob"})));
}

#[test]
fn test_matches_filter_comparison_operators() {
    let doc = json!({"age": 30});
    assert!(matches_filter(&doc, &json!({"age": {"$gt": 25}})));
    assert!(!matches_filter(&doc, &json!({"age": {"$gt": 35}})));
    assert!(matches_filter(&doc, &json!({"age": {"$gte": 30}})));
    assert!(matches_filter(&doc, &json!({"age": {"$lt": 35}})));
    assert!(!matches_filter(&doc, &json!({"age": {"$lt": 30}})));
    assert!(matches_filter(&doc, &json!({"age": {"$lte": 30}})));
    assert!(matches_filter(&doc, &json!({"age": {"$ne": 99}})));
    assert!(!matches_filter(&doc, &json!({"age": {"$ne": 30}})));
}

#[test]
fn test_matches_filter_in_nin() {
    let doc = json!({"status": "active"});
    assert!(matches_filter(
        &doc,
        &json!({"status": {"$in": ["active", "pending"]}})
    ));
    assert!(!matches_filter(
        &doc,
        &json!({"status": {"$in": ["inactive"]}})
    ));
    assert!(matches_filter(
        &doc,
        &json!({"status": {"$nin": ["inactive"]}})
    ));
    assert!(!matches_filter(
        &doc,
        &json!({"status": {"$nin": ["active"]}})
    ));
}

#[test]
fn test_matches_filter_logical_and_or() {
    let doc = json!({"age": 30, "status": "active"});
    assert!(matches_filter(
        &doc,
        &json!({"$and": [{"age": 30}, {"status": "active"}]})
    ));
    assert!(!matches_filter(
        &doc,
        &json!({"$and": [{"age": 30}, {"status": "inactive"}]})
    ));
    assert!(matches_filter(
        &doc,
        &json!({"$or": [{"age": 99}, {"status": "active"}]})
    ));
    assert!(!matches_filter(
        &doc,
        &json!({"$or": [{"age": 99}, {"status": "inactive"}]})
    ));
}

// ---------------------------------------------------------------------------
// @group UpdateTests : Apply-update operator logic (no library needed)
// ---------------------------------------------------------------------------

#[test]
fn test_apply_update_set() {
    let doc = json!({"_id": "abc", "name": "Alice", "age": 30});
    let updated = apply_update(&doc, &json!({"$set": {"age": 31}}));
    assert_eq!(updated["age"], json!(31));
    assert_eq!(updated["name"], json!("Alice"));
    assert_eq!(updated["_id"], json!("abc"));
}

#[test]
fn test_apply_update_unset() {
    let doc = json!({"_id": "abc", "name": "Alice", "tmp": "remove_me"});
    let updated = apply_update(&doc, &json!({"$unset": {"tmp": ""}}));
    assert!(updated.get("tmp").is_none());
    assert_eq!(updated["name"], json!("Alice"));
}

#[test]
fn test_apply_update_inc() {
    let doc = json!({"_id": "abc", "score": 10});
    let updated = apply_update(&doc, &json!({"$inc": {"score": 5}}));
    assert_eq!(updated["score"], json!(15_i64));
}

#[test]
fn test_apply_update_push() {
    let doc = json!({"_id": "abc", "tags": ["rust"]});
    let updated = apply_update(&doc, &json!({"$push": {"tags": "sdk"}}));
    let tags = updated["tags"].as_array().unwrap();
    assert!(tags.contains(&json!("rust")));
    assert!(tags.contains(&json!("sdk")));
}

#[test]
fn test_apply_update_replacement() {
    let doc = json!({"_id": "abc", "name": "Alice"});
    let updated = apply_update(&doc, &json!({"name": "Bob"}));
    assert_eq!(updated["name"], json!("Bob"));
    // _id must be preserved
    assert_eq!(updated["_id"], json!("abc"));
}

// ---------------------------------------------------------------------------
// @group UnitTests : Core document CRUD tests (requires native library)
// ---------------------------------------------------------------------------

#[test]
#[cfg(feature = "integration")]
fn test_insert_one() {
    let (client, _path) = temp_client!();
    let coll = client.database().collection("users");

    let result = coll
        .insert_one(json!({"name": "Alice", "age": 30}))
        .expect("insert_one");

    assert!(!result.inserted_id.is_empty());
}

#[test]
#[cfg(feature = "integration")]
fn test_find_one_by_id() {
    let (client, _path) = temp_client!();
    let coll = client.database().collection("users");

    let result = coll
        .insert_one(json!({"name": "Bob", "age": 25}))
        .expect("insert");
    let doc = coll
        .find_one(Some(&json!({"_id": result.inserted_id})))
        .expect("find_one")
        .expect("doc should exist");

    assert_eq!(doc["name"], json!("Bob"));
    assert_eq!(doc["age"], json!(25));
}

#[test]
#[cfg(feature = "integration")]
fn test_find_all() {
    let (client, _path) = temp_client!();
    let coll = client.database().collection("users");

    coll.insert_one(json!({"name": "Alice"})).unwrap();
    coll.insert_one(json!({"name": "Bob"})).unwrap();

    let docs = coll.find(None).unwrap().all();
    assert!(docs.len() >= 2);
}

#[test]
#[cfg(feature = "integration")]
fn test_update_one() {
    let (client, _path) = temp_client!();
    let coll = client.database().collection("users");

    let res = coll
        .insert_one(json!({"name": "Charlie", "age": 35}))
        .unwrap();
    let update_result = coll
        .update_one(
            &json!({"_id": &res.inserted_id}),
            &json!({"$set": {"age": 36}}),
        )
        .unwrap();

    assert_eq!(update_result.matched_count, 1);
    assert_eq!(update_result.modified_count, 1);

    let doc = coll
        .find_one(Some(&json!({"_id": &res.inserted_id})))
        .unwrap()
        .unwrap();
    assert_eq!(doc["age"], json!(36));
}

#[test]
#[cfg(feature = "integration")]
fn test_delete_one() {
    let (client, _path) = temp_client!();
    let coll = client.database().collection("users");

    let res = coll.insert_one(json!({"name": "Dave"})).unwrap();
    let del = coll
        .delete_one(&json!({"_id": &res.inserted_id}))
        .unwrap();
    assert_eq!(del.deleted_count, 1);

    let doc = coll
        .find_one(Some(&json!({"_id": &res.inserted_id})))
        .unwrap();
    assert!(doc.is_none());
}

#[test]
#[cfg(feature = "integration")]
fn test_count_documents() {
    let (client, _path) = temp_client!();
    let coll = client.database().collection("users");

    coll.insert_one(json!({"name": "Eve"})).unwrap();
    coll.insert_one(json!({"name": "Frank"})).unwrap();

    let count = coll.count_documents(None).unwrap();
    assert!(count >= 2);
}

#[test]
#[cfg(feature = "integration")]
fn test_insert_many() {
    let (client, _path) = temp_client!();
    let coll = client.database().collection("users");

    let docs = vec![
        json!({"name": "User1"}),
        json!({"name": "User2"}),
        json!({"name": "User3"}),
    ];
    let result = coll.insert_many(docs).unwrap();
    assert_eq!(result.inserted_ids.len(), 3);
}

// ---------------------------------------------------------------------------
// @group CursorTests : Cursor limit / skip behaviour (no library needed)
// ---------------------------------------------------------------------------

#[test]
fn test_cursor_limit() {
    use keradb_sdk::Cursor;
    let docs: Vec<serde_json::Value> = (0..10).map(|i| json!({"i": i})).collect();
    let cursor = Cursor::new(docs).limit(5);
    assert_eq!(cursor.all().len(), 5);
}

#[test]
fn test_cursor_skip() {
    use keradb_sdk::Cursor;
    let docs: Vec<serde_json::Value> = (0..10).map(|i| json!({"i": i})).collect();
    let cursor = Cursor::new(docs).skip(7);
    assert_eq!(cursor.all().len(), 3);
}

#[test]
fn test_cursor_limit_and_skip() {
    use keradb_sdk::Cursor;
    let docs: Vec<serde_json::Value> = (0..10).map(|i| json!({"i": i})).collect();
    let result = Cursor::new(docs).skip(2).limit(3).all();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0]["i"], json!(2));
}

#[test]
fn test_cursor_iterate() {
    use keradb_sdk::Cursor;
    let docs: Vec<serde_json::Value> = (0..5).map(|i| json!({"i": i})).collect();
    let collected: Vec<_> = Cursor::new(docs).into_iter().collect();
    assert_eq!(collected.len(), 5);
}

// ---------------------------------------------------------------------------
// @group VectorTests : Vector configuration builder (no library needed)
// ---------------------------------------------------------------------------

#[test]
fn test_vector_config_to_json() {
    let cfg = VectorConfig::new(128)
        .with_distance(Distance::Euclidean)
        .with_m(32)
        .with_ef_construction(300)
        .with_ef_search(100);

    let json_str = cfg.to_json();
    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(v["dimensions"], json!(128));
    assert_eq!(v["distance"], json!("euclidean"));
    assert_eq!(v["m"], json!(32));
    assert_eq!(v["ef_construction"], json!(300));
    assert_eq!(v["ef_search"], json!(100));
}

#[test]
fn test_vector_config_delta_compression() {
    let cfg = VectorConfig::new(64).with_delta_compression();
    let json_str = cfg.to_json();
    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(v["compression"]["mode"], json!("delta"));
}

#[test]
fn test_vector_config_lazy_embedding() {
    let cfg = VectorConfig::new(768).with_lazy_embedding("text-embedding-ada-002");
    let json_str = cfg.to_json();
    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(v["lazy_embedding"], json!(true));
    assert_eq!(v["embedding_model"], json!("text-embedding-ada-002"));
}
