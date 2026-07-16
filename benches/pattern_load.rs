use criterion::{criterion_group, criterion_main, Criterion};
use mmrspace::Pattern;
use std::{fmt::Write as _, hint::black_box};

fn bench_pattern_csv_load_1m_cells(c: &mut Criterion) {
    let full = std::env::var("MMRSPACE_BENCH_PROFILE").as_deref() == Ok("full");
    let n_cells = if full { 1_000_000 } else { 10_000 };
    let dir = tempfile::tempdir().expect("benchmark temp directory");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let mut csv = String::with_capacity(n_cells * 48);
    csv.push_str("x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n");
    for index in 0..n_cells {
        writeln!(
            csv,
            "{},{},{},bench,post,MSH6,true,true",
            index,
            index % 127,
            u8::from(index % 13 == 0)
        )
        .expect("write CSV row");
    }
    std::fs::write(&cells, csv).expect("write CSV fixture");
    std::fs::write(
        &mask,
        format!(
            r#"{{"type":"MultiPolygon","coordinates":[[[[-1,-1],[{},-1],[{},128],[-1,128],[-1,-1]]]]}}"#,
            n_cells + 1,
            n_cells + 1
        ),
    )
    .expect("write mask fixture");

    let mut group = c.benchmark_group("pattern_csv_load");
    group.sample_size(10);
    group.bench_function(format!("pattern_csv_load_{n_cells}_cells"), |b| {
        b.iter(|| {
            black_box(Pattern::from_paths(black_box(&cells), black_box(&mask)))
                .expect("load CSV fixture")
        });
    });
    group.finish();
}

criterion_group!(benches, bench_pattern_csv_load_1m_cells);
criterion_main!(benches);
