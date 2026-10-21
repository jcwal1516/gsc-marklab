use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
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

    #[cfg(feature = "parquet")]
    {
        use arrow::array::{
            ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt8Array,
        };
        use parquet::arrow::ArrowWriter;
        use std::sync::Arc;

        // Canonical input columns mirror the CSV fixture; construction and
        // writing are outside the timed PatternLoader ingestion path.
        let parquet_path = dir.path().join("cells.parquet");
        let columns: Vec<(&str, ArrayRef)> = vec![
            (
                "x_um",
                Arc::new(Float64Array::from_iter_values(
                    (0..n_cells).map(|i| i as f64),
                )),
            ),
            (
                "y_um",
                Arc::new(Float64Array::from_iter_values(
                    (0..n_cells).map(|i| (i % 127) as f64),
                )),
            ),
            (
                "mark",
                Arc::new(UInt8Array::from_iter_values(
                    (0..n_cells).map(|i| u8::from(i % 13 == 0)),
                )),
            ),
            (
                "case_id",
                Arc::new(StringArray::from(vec!["bench"; n_cells])),
            ),
            (
                "timepoint",
                Arc::new(StringArray::from(vec!["post"; n_cells])),
            ),
            (
                "protein",
                Arc::new(StringArray::from(vec!["MSH6"; n_cells])),
            ),
            (
                "valid_tumor",
                Arc::new(BooleanArray::from(vec![true; n_cells])),
            ),
            (
                "valid_ihc",
                Arc::new(BooleanArray::from(vec![true; n_cells])),
            ),
        ];
        let batch = RecordBatch::try_from_iter(columns).expect("canonical Parquet batch");
        let mut writer = ArrowWriter::try_new(
            File::create(&parquet_path).expect("create Parquet fixture"),
            batch.schema(),
            None,
        )
        .expect("Parquet writer");
        writer.write(&batch).expect("write Parquet fixture");
        writer.close().expect("close Parquet fixture");
        drop(batch);
        let expected = loader.load(&parquet_path).expect("load canonical Parquet");
        assert_eq!(expected.len(), n_cells);
        assert_eq!(expected.n_marked(), n_cells.div_ceil(13));
        for i in 0..n_cells {
            assert_eq!(
                (expected.x_um[i], expected.y_um[i], expected.mark[i]),
                (i as f64, (i % 127) as f64, u8::from(i % 13 == 0))
            );
        }
        assert!((expected.window.d_nn_mean_um - 2.0_f64.sqrt()).abs() < 1e-10);
        drop(expected);
        let mut group = c.benchmark_group("pattern_parquet_load");
        group.sample_size(10);
        group.bench_function(format!("pattern_parquet_load_{n_cells}_cells"), |b| {
            b.iter(|| {
                black_box(loader.load(black_box(&parquet_path))).expect("load Parquet fixture")
            });
        });
        group.bench_function(
            format!("pattern_parquet_decode_filter_{n_cells}_cells"),
            |b| {
                b.iter_custom(|iterations| {
                    (0..iterations).fold(Duration::ZERO, |elapsed, _| {
                        let result = loader
                            .load_with_diagnostics(black_box(&parquet_path))
                            .expect("Parquet decode timing");
                        assert_eq!(result.pattern.len(), n_cells);
                        elapsed + result.diagnostics.decode_and_filter
                    })
                });
            },
        );
        group.finish();
    }
}

fn bench_spatial_index_scaling(c: &mut Criterion) {
    let full = std::env::var("MARKLAB_BENCH_PROFILE").as_deref() == Ok("full");
    let sizes: &[usize] = if full {
        &[1_024, 2_048, 4_096, 8_192, 16_384]
    } else {
        &[1_024, 4_096]
    };
    let mut group = c.benchmark_group("indexed_nearest_neighbor_scaling");
    group.sample_size(10);
    for &n in sizes {
        let side = (n as f64).sqrt().ceil() as usize;
        let dir = tempfile::tempdir().expect("benchmark temp directory");
        let cells = dir.path().join("cells.csv");
        let mut csv = BufWriter::new(File::create(&cells).expect("create lattice CSV"));
        writeln!(
            csv,
            "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc"
        )
        .expect("write CSV header");
        for i in 0..n {
            writeln!(
                csv,
                "{},{},{},bench,post,MSH6,true,true",
                i % side,
                i / side,
                u8::from(i % 7 == 0)
            )
            .expect("write lattice row");
        }
        csv.flush().expect("flush lattice CSV");
        drop(csv);
        let extent = side + 1;
        let mask = TumorMask::from_geojson_str(&format!(
            r#"{{"type":"MultiPolygon","coordinates":[[[[-1,-1],[{extent},-1],[{extent},{extent}],[-1,{extent}],[-1,-1]]]]}}"#
        )).expect("lattice mask");
        let loader = PatternLoader::new(&mask);
        group.bench_with_input(BenchmarkId::from_parameter(n), &cells, |b, cells| {
            b.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let result = loader
                        .load_with_diagnostics(black_box(cells))
                        .expect("load lattice");
                    assert_eq!(result.pattern.len(), n);
                    // Every point in this unit lattice has a neighbor exactly one unit away.
                    assert!((result.pattern.window.d_nn_mean_um - 1.0).abs() < 1e-12);
                    // Existing production timing includes index construction and all NN queries,
                    // while CSV decoding and the numeric assertions are outside that timing.
                    elapsed += result.diagnostics.nearest_neighbor;
                    black_box(result);
                }
                elapsed
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_pattern_csv_load_1m_cells,
    bench_spatial_index_scaling
);
criterion_main!(benches);
