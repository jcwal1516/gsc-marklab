#![cfg(all(
    feature = "parquet",
    feature = "dhat-heap",
    not(feature = "allocator-mimalloc")
))]

#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

#[path = "../benches/support/patch_embeddings.rs"]
mod workload;

#[test]
fn patch_embedding_10k_1024_100k_links_peak() {
    let mut prepared = workload::PreparedPatchEmbeddingBenchmark::selected();
    assert_eq!(prepared.patch_count(), 10_000);
    assert_eq!(prepared.dimension(), 1_024);
    assert_eq!(prepared.value_count(), 10_240_000);
    assert_eq!(prepared.assignment_count(), 100_000);
    assert_eq!(prepared.edge_count(), 100_000);
    assert_eq!(prepared.run(), prepared.expected_outcome());

    let _profiler = dhat::Profiler::builder().testing().build();
    let outcome = prepared.run_with_fresh_publication();
    assert_eq!(outcome, prepared.expected_outcome());
    std::hint::black_box(outcome);
    let stats = dhat::HeapStats::get();

    println!(
        "MARKLAB_PATCH_DHAT profile={} table_logical_digest={} link_logical_digest={} numeric_digest={} link_checksum={} current_bytes={} peak_bytes={} peak_limit_bytes={} forbidden_edge_vector_bytes={}",
        prepared.name(),
        outcome.table_logical_digest(),
        outcome.link_logical_digest(),
        outcome.numeric_digest(),
        outcome.link_checksum(),
        stats.curr_bytes,
        stats.max_bytes,
        prepared.dhat_peak_limit(),
        prepared.forbidden_edge_vector_bytes(),
    );

    dhat::assert_eq!(stats.curr_bytes, 0);
    dhat::assert!(
        stats.max_bytes <= prepared.dhat_peak_limit(),
        "tracked peak {} exceeds reviewed cap {}",
        stats.max_bytes,
        prepared.dhat_peak_limit(),
    );
    dhat::assert!(
        stats.max_bytes < prepared.forbidden_edge_vector_bytes(),
        "tracked peak {} does not exclude the forbidden copied-vector payload {}",
        stats.max_bytes,
        prepared.forbidden_edge_vector_bytes(),
    );
}
