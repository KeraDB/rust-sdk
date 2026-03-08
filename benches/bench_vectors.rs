//! Vector operation benchmarks for KeraDB.
//!
//! Compares KeraDB vector search against a naive brute-force linear scan
//! (since SQLite has no built-in ANN support).
//!
//! ## Running
//!
//! Brute-force baseline only (no native lib required):
//! ```text
//! cargo bench --bench bench_vectors
//! ```
//!
//! Full KeraDB HNSW vs brute-force comparison (requires native keradb library):
//! ```text
//! cargo bench --bench bench_vectors --features integration
//! ```

// @group Config        : Benchmark constants and vector helpers
// @group BruteForce    : Naive linear scan baseline (no index)
// @group InsertVectors : Vector insertion benchmarks
// @group SearchVectors : ANN search benchmarks
// @group CompressedVec : Delta-compressed vector collection benchmarks

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// ---------------------------------------------------------------------------
// @group Config : Benchmark constants and vector helpers
// ---------------------------------------------------------------------------

const VECTOR_DIM: usize = 128;
const SMALL_CORPUS: usize = 500;
const LARGE_CORPUS: usize = 5_000;
const K: usize = 10;

/// Generate a deterministic normalised vector from a seed.
fn make_embedding(dim: usize, seed: u64) -> Vec<f32> {
    let mut state = seed.wrapping_add(1);
    let raw: Vec<f32> = (0..dim)
        .map(|_| {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            ((state >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0
        })
        .collect();
    let norm: f32 = raw.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    raw.iter().map(|x| x / norm).collect()
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// ---------------------------------------------------------------------------
// @group BruteForce : Naive linear scan baseline
// ---------------------------------------------------------------------------

/// Brute-force k-NN: scan the entire corpus for every query.
fn brute_force_search(corpus: &[Vec<f32>], query: &[f32], k: usize) -> Vec<(usize, f32)> {
    let mut scores: Vec<(usize, f32)> = corpus
        .iter()
        .enumerate()
        .map(|(i, v)| (i, cosine_similarity(v, query)))
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores.truncate(k);
    scores
}

fn bench_brute_force_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_search_brute_force");
    group.measurement_time(Duration::from_secs(10));

    for corpus_size in [SMALL_CORPUS, LARGE_CORPUS] {
        let corpus: Vec<Vec<f32>> = (0..corpus_size)
            .map(|i| make_embedding(VECTOR_DIM, i as u64))
            .collect();
        let query = make_embedding(VECTOR_DIM, 99_999);

        group.throughput(Throughput::Elements(corpus_size as u64));
        group.bench_with_input(
            BenchmarkId::new("linear_scan", corpus_size),
            &corpus_size,
            |b, _| {
                b.iter(|| {
                    black_box(brute_force_search(&corpus, &query, K))
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group InsertVectors : Vector insertion benchmarks
// ---------------------------------------------------------------------------

#[cfg(feature = "integration")]
fn bench_vector_insert(c: &mut Criterion) {
    use keradb_sdk::{connect, Distance, VectorConfig};
    use serde_json::json;

    let mut group = c.benchmark_group("vector_insert");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("keradb_hnsw", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("bench_vec.ndb");
            let client = connect(path.to_str().unwrap()).expect("connect");

            let cfg = VectorConfig::new(VECTOR_DIM)
                .with_distance(Distance::Cosine)
                .with_m(16)
                .with_ef_construction(200);
            client
                .create_vector_collection("vecs", &cfg)
                .expect("create");

            for i in 0..100u64 {
                let emb = make_embedding(VECTOR_DIM, i);
                client
                    .insert_vector("vecs", &emb, Some(&json!({"index": i})))
                    .expect("insert_vector");
            }
            black_box(())
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// @group SearchVectors : HNSW search vs brute-force
// ---------------------------------------------------------------------------

#[cfg(feature = "integration")]
fn bench_vector_search(c: &mut Criterion) {
    use keradb_sdk::{connect, Distance, VectorConfig};
    use serde_json::json;

    let mut group = c.benchmark_group("vector_search");
    group.measurement_time(Duration::from_secs(15));

    let query = make_embedding(VECTOR_DIM, 99_999);

    // Brute-force baseline for comparison
    {
        let corpus: Vec<Vec<f32>> = (0..SMALL_CORPUS)
            .map(|i| make_embedding(VECTOR_DIM, i as u64))
            .collect();
        let q = query.clone();
        group.bench_function(
            BenchmarkId::new("brute_force", SMALL_CORPUS),
            |b| {
                b.iter(|| black_box(brute_force_search(&corpus, &q, K)));
            },
        );
    }

    // KeraDB HNSW
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bench_search.ndb");
        let client = connect(path.to_str().unwrap()).expect("connect");

        let cfg = VectorConfig::new(VECTOR_DIM)
            .with_distance(Distance::Cosine)
            .with_m(16)
            .with_ef_construction(200)
            .with_ef_search(50);
        client.create_vector_collection("vecs", &cfg).expect("create");

        for i in 0..SMALL_CORPUS as u64 {
            let emb = make_embedding(VECTOR_DIM, i);
            client
                .insert_vector("vecs", &emb, Some(&json!({"index": i})))
                .expect("insert_vector");
        }

        let q = query.clone();
        group.bench_function(
            BenchmarkId::new("keradb_hnsw", SMALL_CORPUS),
            |b| {
                b.iter(|| {
                    black_box(
                        client
                            .vector_search("vecs", &q, K)
                            .expect("vector_search"),
                    )
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// @group CompressedVec : Delta-compressed collection search
// ---------------------------------------------------------------------------

#[cfg(feature = "integration")]
fn bench_compressed_insert(c: &mut Criterion) {
    use keradb_sdk::{connect, Distance, VectorConfig};
    use serde_json::json;

    let mut group = c.benchmark_group("vector_insert_compressed");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    group.bench_function("keradb_delta", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("bench_delta.ndb");
            let client = connect(path.to_str().unwrap()).expect("connect");

            let cfg = VectorConfig::new(VECTOR_DIM)
                .with_distance(Distance::Cosine)
                .with_delta_compression();
            client.create_vector_collection("vecs", &cfg).expect("create");

            for i in 0..100u64 {
                let emb = make_embedding(VECTOR_DIM, i);
                client
                    .insert_vector("vecs", &emb, Some(&json!({"index": i})))
                    .expect("insert_vector");
            }
            black_box(())
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// @group SearchVectors : ef_search parameter tuning
// ---------------------------------------------------------------------------

#[cfg(feature = "integration")]
fn bench_ef_search_tuning(c: &mut Criterion) {
    use keradb_sdk::{connect, Distance, VectorConfig};
    use serde_json::json;

    let mut group = c.benchmark_group("ef_search_tuning");
    group.measurement_time(Duration::from_secs(10));

    let query = make_embedding(VECTOR_DIM, 99_999);

    for ef in [10u32, 50, 100, 200] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("bench_ef{}.ndb", ef));
        let client = connect(path.to_str().unwrap()).expect("connect");

        let cfg = VectorConfig::new(VECTOR_DIM)
            .with_distance(Distance::Cosine)
            .with_ef_construction(200)
            .with_ef_search(ef);
        client.create_vector_collection("vecs", &cfg).expect("create");
        for i in 0..SMALL_CORPUS as u64 {
            let emb = make_embedding(VECTOR_DIM, i);
            client
                .insert_vector("vecs", &emb, Some(&json!({"index": i})))
                .expect("insert");
        }

        let q = query.clone();
        group.bench_with_input(BenchmarkId::new("ef_search", ef), &ef, |b, _| {
            b.iter(|| {
                black_box(
                    client.vector_search("vecs", &q, K).expect("search"),
                )
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Register benchmark groups
// ---------------------------------------------------------------------------

/// No-op placeholder so `keradb_benches` is always a valid symbol.
#[allow(dead_code)]
fn bench_keradb_noop(_c: &mut Criterion) {}

// Always-available benchmarks (no native lib)
criterion_group!(base_benches, bench_brute_force_search);

// KeraDB-specific benchmarks (require native lib)
#[cfg(feature = "integration")]
criterion_group!(
    keradb_benches,
    bench_vector_insert,
    bench_vector_search,
    bench_compressed_insert,
    bench_ef_search_tuning,
);

// Without the native lib just register the noop so criterion_main! compiles.
#[cfg(not(feature = "integration"))]
criterion_group!(keradb_benches, bench_keradb_noop);

criterion_main!(base_benches, keradb_benches);
