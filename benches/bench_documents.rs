//! Document operation benchmarks: KeraDB vs SQLite
//!
//! Mirrors the Python SDK's `benchmark_documents.py`.
//!
//! ## Running
//!
//! SQLite-only benchmarks (no native lib required):
//! ```text
//! cargo bench --bench bench_documents
//! ```
//!
//! Full KeraDB vs SQLite comparison (requires native keradb library):
//! ```text
//! cargo bench --bench bench_documents --features integration
//! ```
//!
//! HTML report is written to `target/criterion/`.

// @group Config      : Benchmark constants
// @group SQLiteSetup : SQLite fixture helpers
// @group KeraDBSetup : KeraDB fixture helpers (feature = integration)
// @group InsertBench : Single and batch insert benchmarks
// @group FindBench   : Find by ID and find-all benchmarks
// @group UpdateBench : Update benchmarks
// @group DeleteBench : Delete benchmarks
// @group CountBench  : Count benchmarks

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rusqlite::{params, Connection};
#[cfg(feature = "integration")]
use serde_json::json;
use std::time::Duration;

// ---------------------------------------------------------------------------
// @group Config : Benchmark constants
// ---------------------------------------------------------------------------

const NUM_DOCS: usize = 1_000;
const BATCH_SIZE: usize = 100;


// ---------------------------------------------------------------------------
// @group SQLiteSetup : SQLite fixture helpers
// ---------------------------------------------------------------------------

/// Open an in-memory SQLite database with the documents table and indices.
fn sqlite_setup() -> Connection {
    let conn = Connection::open_in_memory().expect("sqlite in-memory");
    conn.execute_batch(
        "CREATE TABLE documents (
            id      TEXT PRIMARY KEY,
            name    TEXT NOT NULL,
            age     INTEGER NOT NULL,
            email   TEXT NOT NULL,
            active  INTEGER NOT NULL,
            metadata TEXT NOT NULL
        );
        CREATE INDEX idx_name ON documents(name);
        ",
    )
    .expect("create table");
    conn
}

/// Open an on-disk SQLite database in a temp file.
fn sqlite_ondisk_setup() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bench.db");
    let conn = Connection::open(&path).expect("sqlite ondisk");
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         CREATE TABLE documents (
             id      TEXT PRIMARY KEY,
             name    TEXT NOT NULL,
             age     INTEGER NOT NULL,
             email   TEXT NOT NULL,
             active  INTEGER NOT NULL,
             metadata TEXT NOT NULL
         );
         CREATE INDEX idx_name ON documents(name);",
    )
    .expect("create table ondisk");
    (conn, dir)
}

/// Insert `n` rows into SQLite and return the last inserted id.
fn sqlite_seed(conn: &Connection, n: usize) -> String {
    let mut stmt = conn
        .prepare(
            "INSERT INTO documents (id, name, age, email, active, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .expect("prepare");
    for i in 0..n {
        stmt.execute(params![
            format!("doc{}", i),
            format!("User{}", i),
            20 + (i % 50),
            format!("user{}@example.com", i),
            i % 2,
            "{}"
        ])
        .expect("insert");
    }
    format!("doc{}", n - 1)
}

// ---------------------------------------------------------------------------
// @group KeraDBSetup : KeraDB fixture helpers (requires native library)
// ---------------------------------------------------------------------------

#[cfg(feature = "integration")]
fn keradb_setup() -> (keradb_sdk::Client, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bench.ndb");
    let client = keradb_sdk::connect(path.to_str().unwrap()).expect("keradb connect");
    (client, dir)
}

#[cfg(feature = "integration")]
fn keradb_seed(client: &keradb_sdk::Client, n: usize) -> String {
    let coll = client.database().collection("users");
    let mut last_id = String::new();
    for i in 0..n {
        let r = coll
            .insert_one(json!({
                "name": format!("User{}", i),
                "age": 20 + (i % 50),
                "email": format!("user{}@example.com", i),
                "active": i % 2 == 0
            }))
            .expect("insert");
        last_id = r.inserted_id;
    }
    last_id
}

// ---------------------------------------------------------------------------
// @group InsertBench : Single document insert
// ---------------------------------------------------------------------------

fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_one");
    group.measurement_time(Duration::from_secs(10));

    // SQLite in-memory (no fsync)
    group.bench_function("sqlite/inmem", |b| {
        let conn = sqlite_setup();
        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            conn.execute(
                "INSERT OR REPLACE INTO documents (id, name, age, email, active, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    format!("doc{}", counter),
                    format!("User{}", counter),
                    black_box(20 + counter % 50),
                    format!("user{}@example.com", counter),
                    counter % 2,
                    "{}"
                ],
            )
            .expect("insert")
        });
    });

    // SQLite on-disk with WAL (apples-to-apples with KeraDB)
    group.bench_function("sqlite/ondisk", |b| {
        let (conn, _dir) = sqlite_ondisk_setup();
        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            conn.execute(
                "INSERT OR REPLACE INTO documents (id, name, age, email, active, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    format!("doc{}", counter),
                    format!("User{}", counter),
                    black_box(20 + counter % 50),
                    format!("user{}@example.com", counter),
                    counter % 2,
                    "{}"
                ],
            )
            .expect("insert")
        });
    });

    // KeraDB on-disk (requires native library)
    #[cfg(feature = "integration")]
    {
        group.bench_function("keradb", |b| {
            let (client, _dir) = keradb_setup();
            let coll = client.database().collection("users");
            let mut counter = 0usize;
            b.iter(|| {
                counter += 1;
                coll.insert_one(black_box(json!({
                    "name": format!("User{}", counter),
                    "age": 20 + counter % 50,
                    "email": format!("user{}@example.com", counter),
                    "active": counter % 2 == 0
                })))
                .expect("insert")
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group InsertBench : Batch insert
// ---------------------------------------------------------------------------

fn bench_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_batch");
    group.throughput(Throughput::Elements(BATCH_SIZE as u64));
    group.measurement_time(Duration::from_secs(15));

    // SQLite in-memory with transaction (realistic batch scenario)
    group.bench_with_input(
        BenchmarkId::new("sqlite/inmem", BATCH_SIZE),
        &BATCH_SIZE,
        |b, &size| {
            b.iter(|| {
                let conn = sqlite_setup();
                conn.execute_batch("BEGIN").expect("begin");
                for i in 0..size {
                    conn.execute(
                        "INSERT INTO documents (id, name, age, email, active, metadata)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            format!("doc{}", i),
                            format!("User{}", i),
                            20 + i % 50,
                            format!("user{}@example.com", i),
                            i % 2,
                            "{}"
                        ],
                    )
                    .expect("insert");
                }
                conn.execute_batch("COMMIT").expect("commit");
            });
        },
    );

    // SQLite on-disk with transaction (apples-to-apples with KeraDB)
    group.bench_with_input(
        BenchmarkId::new("sqlite/ondisk", BATCH_SIZE),
        &BATCH_SIZE,
        |b, &size| {
            b.iter(|| {
                let (conn, _dir) = sqlite_ondisk_setup();
                conn.execute_batch("BEGIN").expect("begin");
                for i in 0..size {
                    conn.execute(
                        "INSERT INTO documents (id, name, age, email, active, metadata)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            format!("doc{}", i),
                            format!("User{}", i),
                            20 + i % 50,
                            format!("user{}@example.com", i),
                            i % 2,
                            "{}"
                        ],
                    )
                    .expect("insert");
                }
                conn.execute_batch("COMMIT").expect("commit");
            });
        },
    );

    // KeraDB on-disk
    #[cfg(feature = "integration")]
    group.bench_with_input(
        BenchmarkId::new("keradb", BATCH_SIZE),
        &BATCH_SIZE,
        |b, &size| {
            b.iter(|| {
                let (client, _dir) = keradb_setup();
                let coll = client.database().collection("users");
                let docs: Vec<serde_json::Value> = (0..size)
                    .map(|i| {
                        json!({
                            "name": format!("User{}", i),
                            "age": 20 + i % 50,
                            "email": format!("user{}@example.com", i),
                            "active": i % 2 == 0
                        })
                    })
                    .collect();
                coll.insert_many(docs).expect("insert_many")
            });
        },
    );

    group.finish();
}

// ---------------------------------------------------------------------------
// @group FindBench : Find by ID
// ---------------------------------------------------------------------------

fn bench_find_by_id(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_by_id");
    group.measurement_time(Duration::from_secs(10));

    // SQLite in-memory — prepare statement outside iter to exclude compile overhead
    group.bench_function("sqlite/inmem", |b| {
        let conn = sqlite_setup();
        sqlite_seed(&conn, 100);
        let mut stmt = conn
            .prepare("SELECT * FROM documents WHERE id = ?1")
            .expect("prepare");
        b.iter(|| {
            stmt.query_row(params!["doc50"], |_| Ok(()))
                .expect("find")
        });
    });

    // SQLite on-disk
    group.bench_function("sqlite/ondisk", |b| {
        let (conn, _dir) = sqlite_ondisk_setup();
        sqlite_seed(&conn, 100);
        let mut stmt = conn
            .prepare("SELECT * FROM documents WHERE id = ?1")
            .expect("prepare ondisk");
        b.iter(|| {
            stmt.query_row(params!["doc50"], |_| Ok(()))
                .expect("find ondisk")
        });
    });

    // KeraDB
    #[cfg(feature = "integration")]
    {
        group.bench_function("keradb", |b| {
            let (client, _dir) = keradb_setup();
            let coll = client.database().collection("users");
            let id = keradb_seed(&client, 100);
            b.iter(|| {
                coll.find_one(Some(&json!({"_id": &id})))
                    .expect("find_one")
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group FindBench : Find all documents
// ---------------------------------------------------------------------------

fn bench_find_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_all");
    group.throughput(Throughput::Elements(100));
    group.measurement_time(Duration::from_secs(10));

    // SQLite in-memory — prepare statement outside iter
    group.bench_function("sqlite/inmem", |b| {
        let conn = sqlite_setup();
        sqlite_seed(&conn, 100);
        let mut stmt = conn
            .prepare("SELECT * FROM documents")
            .expect("prepare");
        b.iter(|| {
            let rows: Vec<_> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .expect("query")
                .collect();
            black_box(rows)
        });
    });

    // SQLite on-disk
    group.bench_function("sqlite/ondisk", |b| {
        let (conn, _dir) = sqlite_ondisk_setup();
        sqlite_seed(&conn, 100);
        let mut stmt = conn
            .prepare("SELECT * FROM documents")
            .expect("prepare ondisk");
        b.iter(|| {
            let rows: Vec<_> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .expect("query ondisk")
                .collect();
            black_box(rows)
        });
    });

    // KeraDB
    #[cfg(feature = "integration")]
    {
        group.bench_function("keradb", |b| {
            let (client, _dir) = keradb_setup();
            let coll = client.database().collection("users");
            keradb_seed(&client, 100);
            b.iter(|| black_box(coll.find(None).expect("find").all()));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group UpdateBench : Update a single document
// ---------------------------------------------------------------------------

fn bench_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_one");
    group.measurement_time(Duration::from_secs(10));

    // SQLite in-memory
    group.bench_function("sqlite/inmem", |b| {
        let conn = sqlite_setup();
        sqlite_seed(&conn, 1);
        b.iter(|| {
            conn.execute(
                "UPDATE documents SET age = ?1 WHERE id = ?2",
                params![black_box(31), "doc0"],
            )
            .expect("update")
        });
    });

    // SQLite on-disk
    group.bench_function("sqlite/ondisk", |b| {
        let (conn, _dir) = sqlite_ondisk_setup();
        sqlite_seed(&conn, 1);
        b.iter(|| {
            conn.execute(
                "UPDATE documents SET age = ?1 WHERE id = ?2",
                params![black_box(31), "doc0"],
            )
            .expect("update ondisk")
        });
    });

    // KeraDB
    #[cfg(feature = "integration")]
    {
        group.bench_function("keradb", |b| {
            let (client, _dir) = keradb_setup();
            let coll = client.database().collection("users");
            let id = keradb_seed(&client, 1);
            b.iter(|| {
                coll.update_one(
                    &json!({"_id": &id}),
                    &black_box(json!({"$set": {"age": 31}})),
                )
                .expect("update_one")
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group DeleteBench : Delete a single document
// ---------------------------------------------------------------------------

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_one");
    group.measurement_time(Duration::from_secs(10));

    // SQLite in-memory — insert+delete in same iter to match KeraDB semantics
    group.bench_function("sqlite/inmem", |b| {
        let conn = sqlite_setup();
        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let id = format!("tmp{}", counter);
            conn.execute(
                "INSERT INTO documents (id, name, age, email, active, metadata)
                 VALUES (?1, 'Temp', 25, 'tmp@example.com', 1, '{}')",
                params![&id],
            )
            .expect("insert");
            conn.execute("DELETE FROM documents WHERE id = ?1", params![&id])
                .expect("delete")
        });
    });

    // SQLite on-disk
    group.bench_function("sqlite/ondisk", |b| {
        let (conn, _dir) = sqlite_ondisk_setup();
        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let id = format!("tmp{}", counter);
            conn.execute(
                "INSERT INTO documents (id, name, age, email, active, metadata)
                 VALUES (?1, 'Temp', 25, 'tmp@example.com', 1, '{}')",
                params![&id],
            )
            .expect("insert ondisk");
            conn.execute("DELETE FROM documents WHERE id = ?1", params![&id])
                .expect("delete ondisk")
        });
    });

    // KeraDB
    #[cfg(feature = "integration")]
    {
        group.bench_function("keradb", |b| {
            let (client, _dir) = keradb_setup();
            let coll = client.database().collection("users");
            b.iter(|| {
                let id = coll
                    .insert_one(json!({"name": "Temp", "age": 25}))
                    .expect("insert")
                    .inserted_id;
                coll.delete_one(&json!({"_id": &id})).expect("delete_one")
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group CountBench : Count documents
// ---------------------------------------------------------------------------

fn bench_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("count_documents");
    group.measurement_time(Duration::from_secs(10));

    // SQLite in-memory
    group.bench_function("sqlite/inmem", |b| {
        let conn = sqlite_setup();
        sqlite_seed(&conn, 100);
        b.iter(|| {
            conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get::<_, i64>(0))
                .expect("count")
        });
    });

    // SQLite on-disk
    group.bench_function("sqlite/ondisk", |b| {
        let (conn, _dir) = sqlite_ondisk_setup();
        sqlite_seed(&conn, 100);
        b.iter(|| {
            conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get::<_, i64>(0))
                .expect("count ondisk")
        });
    });

    // KeraDB
    #[cfg(feature = "integration")]
    {
        group.bench_function("keradb", |b| {
            let (client, _dir) = keradb_setup();
            let coll = client.database().collection("users");
            keradb_seed(&client, 100);
            b.iter(|| {
                black_box(coll.count_documents(None).expect("count"))
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group FindBench : Bulk throughput — NUM_DOCS inserts then scan
// ---------------------------------------------------------------------------

fn bench_bulk_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_throughput");
    group.throughput(Throughput::Elements(NUM_DOCS as u64));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    // SQLite in-memory with transaction (realistic bulk scenario)
    group.bench_function("sqlite/inmem", |b| {
        b.iter(|| {
            let conn = sqlite_setup();
            conn.execute_batch("BEGIN").expect("begin");
            for i in 0..NUM_DOCS {
                conn.execute(
                    "INSERT INTO documents (id, name, age, email, active, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        format!("doc{}", i),
                        format!("User{}", i),
                        20 + i % 50,
                        format!("user{}@example.com", i),
                        i % 2,
                        "{}"
                    ],
                )
                .expect("insert");
            }
            conn.execute_batch("COMMIT").expect("commit");
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
                .expect("count");
            black_box(count)
        });
    });

    // SQLite on-disk with transaction
    group.bench_function("sqlite/ondisk", |b| {
        b.iter(|| {
            let (conn, _dir) = sqlite_ondisk_setup();
            conn.execute_batch("BEGIN").expect("begin");
            for i in 0..NUM_DOCS {
                conn.execute(
                    "INSERT INTO documents (id, name, age, email, active, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        format!("doc{}", i),
                        format!("User{}", i),
                        20 + i % 50,
                        format!("user{}@example.com", i),
                        i % 2,
                        "{}"
                    ],
                )
                .expect("insert ondisk");
            }
            conn.execute_batch("COMMIT").expect("commit");
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
                .expect("count");
            black_box(count)
        });
    });

    // KeraDB on-disk
    #[cfg(feature = "integration")]
    {
        group.bench_function("keradb", |b| {
            b.iter(|| {
                let (client, _dir) = keradb_setup();
                let coll = client.database().collection("users");
                for i in 0..NUM_DOCS {
                    coll.insert_one(json!({
                        "name": format!("User{}", i),
                        "age": 20 + i % 50,
                        "email": format!("user{}@example.com", i),
                        "active": i % 2 == 0
                    }))
                    .expect("insert");
                }
                black_box(coll.count_documents(None).expect("count"))
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Register all benchmark groups
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_insert,
    bench_insert_batch,
    bench_find_by_id,
    bench_find_all,
    bench_update,
    bench_delete,
    bench_count,
    bench_bulk_throughput,
);
criterion_main!(benches);
