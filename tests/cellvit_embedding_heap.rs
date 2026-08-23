#![cfg(all(
    feature = "csv",
    feature = "parquet",
    feature = "dhat-heap",
    not(feature = "allocator-mimalloc")
))]

#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

#[allow(
    dead_code,
    reason = "the shared benchmark support also contains the full profile"
)]
#[path = "../benches/support/cell_embeddings.rs"]
mod workload;

#[test]
fn embedding_10k_1280_peak() {
    let prepared = workload::PreparedEmbeddingBenchmark::smoke();
    assert_eq!(prepared.row_count(), 10_000);
    assert_eq!(prepared.dimension(), 1_280);

    let _profiler = dhat::Profiler::builder().testing().build();
    let outcome = prepared.run_with_physical_publication();
    assert_eq!(outcome, prepared.expected_outcome());
    std::hint::black_box(outcome);
    let stats = dhat::HeapStats::get();

    println!(
        "MARKLAB_DHAT profile=10k_x_1280 logical_digest={} numeric_digest={} current_bytes={} peak_bytes={} peak_limit_bytes={}",
        outcome.logical_digest(),
        outcome.numeric_digest(),
        stats.curr_bytes,
        stats.max_bytes,
        prepared.dhat_peak_limit(),
    );

    dhat::assert_eq!(stats.curr_bytes, 0);
    dhat::assert!(
        stats.max_bytes <= prepared.dhat_peak_limit(),
        "tracked peak {} exceeds declared cap {}",
        stats.max_bytes,
        prepared.dhat_peak_limit(),
    );
}
