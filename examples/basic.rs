//! Basic usage example for the KeraDB Rust SDK.
//!
//! Run with:
//! ```text
//! cargo run --example basic
//! ```

// @group ExampleSetup : Database setup and teardown
// @group ExampleCRUD  : Document CRUD demonstration
// @group ExampleQuery : Filter and cursor usage

use keradb_sdk::connect;
use serde_json::json;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "example_basic.ndb";

    // @group ExampleSetup : Open / create database
    println!("=== KeraDB Rust SDK – Basic Example ===\n");

    let mut client = connect(db_path)?;
    let db = client.database();
    let users = db.collection("users");

    // @group ExampleCRUD : Insert documents

    println!("--- Insert ---");
    let r1 = users.insert_one(json!({
        "name": "Alice",
        "age": 30,
        "email": "alice@example.com",
        "city": "New York"
    }))?;
    println!("Inserted Alice  → {}", r1.inserted_id);

    let r2 = users.insert_one(json!({
        "name": "Bob",
        "age": 25,
        "email": "bob@example.com",
        "city": "London"
    }))?;
    println!("Inserted Bob    → {}", r2.inserted_id);

    let r3 = users.insert_one(json!({
        "name": "Charlie",
        "age": 35,
        "email": "charlie@example.com",
        "city": "Tokyo"
    }))?;
    println!("Inserted Charlie → {}", r3.inserted_id);

    // insert_many
    let many = users.insert_many(vec![
        json!({"name": "Dave",  "age": 28, "city": "Paris"}),
        json!({"name": "Eve",   "age": 22, "city": "Berlin"}),
        json!({"name": "Frank", "age": 40, "city": "New York"}),
    ])?;
    println!("Inserted 3 more  → {:?}", many.inserted_ids);

    // @group ExampleCRUD : Find

    println!("\n--- Find ---");

    // Find by _id
    if let Some(doc) = users.find_one(Some(&json!({"_id": &r1.inserted_id})))? {
        println!("find_one Alice: {}", doc["name"]);
    }

    // Find all
    let all = users.find(None)?.all();
    println!("Total documents: {}", all.len());

    // @group ExampleQuery : Filtered queries

    println!("\n--- Filtered queries ---");

    // Age >= 30
    let seniors = users
        .find(Some(&json!({"age": {"$gte": 30}})))?
        .all();
    println!(
        "Age >= 30: {}",
        seniors
            .iter()
            .map(|d| d["name"].as_str().unwrap_or("?"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // City = New York
    let ny = users
        .find(Some(&json!({"city": "New York"})))?
        .all();
    println!(
        "In New York: {}",
        ny.iter()
            .map(|d| d["name"].as_str().unwrap_or("?"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Cursor: first 2, skip 1
    let paged = users.find(None)?.skip(1).limit(2).all();
    println!("Skip 1, limit 2: {} docs", paged.len());

    // count_documents
    let count = users.count_documents(None)?;
    println!("count_documents: {}", count);

    // @group ExampleCRUD : Update

    println!("\n--- Update ---");

    let upd = users.update_one(
        &json!({"_id": &r1.inserted_id}),
        &json!({"$set": {"age": 31}}),
    )?;
    println!("update_one → matched={}, modified={}", upd.matched_count, upd.modified_count);

    // $inc
    users.update_one(
        &json!({"_id": &r2.inserted_id}),
        &json!({"$inc": {"age": 1}}),
    )?;
    let bob = users
        .find_one(Some(&json!({"_id": &r2.inserted_id})))?
        .unwrap();
    println!("Bob age after $inc: {}", bob["age"]);

    // update_many
    let upd_many = users.update_many(
        &json!({"city": "New York"}),
        &json!({"$set": {"country": "USA"}}),
    )?;
    println!(
        "update_many NYC → matched={}, modified={}",
        upd_many.matched_count, upd_many.modified_count
    );

    // @group ExampleCRUD : Delete

    println!("\n--- Delete ---");

    let del = users.delete_one(&json!({"_id": &r3.inserted_id}))?;
    println!("delete_one Charlie → deleted={}", del.deleted_count);

    let del_many = users.delete_many(&json!({"age": {"$lt": 25}}))?;
    println!("delete_many age < 25 → deleted={}", del_many.deleted_count);

    // List collections
    println!("\n--- Collections ---");
    let col_names = db.list_collection_names()?;
    println!("Collections: {:?}", col_names);

    // Sync and close
    client.sync()?;
    client.close();
    println!("\nDatabase closed ✓");

    // Cleanup example file
    let _ = fs::remove_file(db_path);

    Ok(())
}
