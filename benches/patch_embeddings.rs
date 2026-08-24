use std::{hint::black_box, time::Duration};

use criterion::{criterion_group, criterion_main, Criterion, SamplingMode, Throughput};

#[path = "support/patch_embeddings.rs"]
mod workload;

fn bench_patch_embeddings(c: &mut Criterion) {
    let prepared = workload::PreparedPatchEmbeddingBenchmark::selected();
    let expected = prepared.expected_outcome();
    println!(
        "MARKLAB_PATCH_BENCH profile={} patches={} dimension={} values={} assignments={} edges={} table_logical_digest={} link_logical_digest={} numeric_digest={} link_checksum={}",
        prepared.name(),
        prepared.patch_count(),
        prepared.dimension(),
        prepared.value_count(),
        prepared.assignment_count(),
        prepared.edge_count(),
        expected.table_logical_digest(),
        expected.link_logical_digest(),
        expected.numeric_digest(),
        expected.link_checksum(),
    );

    let mut group = c.benchmark_group("patch_embeddings");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Elements(
        prepared
            .value_count()
            .checked_add(prepared.edge_count())
            .expect("benchmark throughput count"),
    ));
    group.bench_function(prepared.name(), |b| {
        b.iter(|| {
            let outcome = prepared.run();
            assert_eq!(
                outcome,
                expected,
                "unexpected patch benchmark outcome: table={} link={} numeric={} link_checksum={}",
                outcome.table_logical_digest(),
                outcome.link_logical_digest(),
                outcome.numeric_digest(),
                outcome.link_checksum(),
            );
            black_box(outcome)
        });
    });
    group.finish();
}

criterion_group!(benches, bench_patch_embeddings);
criterion_main!(benches);
