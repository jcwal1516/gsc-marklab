use criterion::{criterion_group, criterion_main, Criterion};
use marklab::{PatternLoader, TumorMask};
use std::{
    fs::File,
    hint::black_box,
    io::{BufWriter, Write},
    time::Duration,
};

fn bench_pattern_csv_load_1m_cells(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let n_cells = if full { 1_000_000 } else { 10_000 };
    let dir = tempfile::tempdir().expect("benchmark temp directory");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let mut csv = BufWriter::new(File::create(&cells).expect("create CSV fixture"));
    writeln!(
        csv,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc"
    )
    .expect("write CSV header");
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
    csv.flush().expect("flush CSV fixture");
    drop(csv);
    std::fs::write(
        &mask,
        format!(
            r#"{{"type":"MultiPolygon","coordinates":[[[[-1,-1],[{},-1],[{},128],[-1,128],[-1,-1]]]]}}"#,
            n_cells + 1,
            n_cells + 1
        ),
    )
    .expect("write mask fixture");
    let mask_text = std::fs::read_to_string(&mask).expect("read mask fixture");
    let mask = TumorMask::from_geojson_str(&mask_text).expect("parse mask fixture");
    let loader = PatternLoader::new(&mask);

    let mut group = c.benchmark_group("pattern_csv_load");
    group.sample_size(10);
    group.bench_function(format!("pattern_csv_load_{n_cells}_cells"), |b| {
        b.iter(|| black_box(loader.load(black_box(&cells))).expect("load CSV fixture"));
    });
    group.bench_function(format!("pattern_csv_decode_filter_{n_cells}_cells"), |b| {
        b.iter_custom(|iterations| {
            (0..iterations).fold(Duration::ZERO, |elapsed, _| {
                let result = loader
                    .load_with_diagnostics(black_box(&cells))
                    .expect("load CSV fixture for decode timing");
                assert_eq!(result.pattern.len(), n_cells);
                elapsed + result.diagnostics.decode_and_filter
            })
        });
    });
    group.bench_function(format!("pattern_nearest_neighbor_{n_cells}_cells"), |b| {
        b.iter_custom(|iterations| {
            (0..iterations).fold(Duration::ZERO, |elapsed, _| {
                let result = loader
                    .load_with_diagnostics(black_box(&cells))
                    .expect("load CSV fixture for nearest-neighbor timing");
                assert_eq!(result.pattern.len(), n_cells);
                elapsed + result.diagnostics.nearest_neighbor
            })
        });
    });
    group.finish();
}

criterion_group!(benches, bench_pattern_csv_load_1m_cells);
criterion_main!(benches);
