use std::{hint::black_box, time::Duration};

use criterion::{criterion_group, criterion_main, Criterion, SamplingMode, Throughput};

#[path = "support/cell_embeddings.rs"]
mod workload;

fn bench_cell_embeddings(c: &mut Criterion) {
    let prepared = workload::PreparedEmbeddingBenchmark::selected();
    let expected = prepared.expected_outcome();
    println!(
        "MARKLAB_BENCH profile={} rows={} dimension={} values={} logical_digest={} numeric_digest={}",
        prepared.name(),
        prepared.row_count(),
        prepared.dimension(),
        prepared.value_count(),
        expected.logical_digest(),
        expected.numeric_digest(),
    );

    let mut group = c.benchmark_group("cell_embeddings");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Elements(prepared.value_count()));
    group.bench_function(prepared.name(), |b| {
        b.iter(|| {
            let outcome = prepared.run();
            assert_eq!(
                outcome,
                expected,
                "unexpected benchmark outcome: logical_digest={} numeric_digest={}",
                outcome.logical_digest(),
                outcome.numeric_digest(),
            );
            black_box(outcome)
        });
    });
    group.finish();
}

criterion_group!(benches, bench_cell_embeddings);
criterion_main!(benches);
