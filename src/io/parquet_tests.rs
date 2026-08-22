#![cfg(feature = "parquet")]

use std::fs;

use crate::{
    data::PatternMeta,
    geom::mask::TumorMask,
    io::{
        load_pattern_path,
        parquet::{load_pattern_parquet_with_diagnostics, write_filtered_pattern_export_parquet},
    },
    Pattern,
};

#[test]
fn filtered_parquet_export_preserves_supported_pattern_fields() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cells.parquet");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1, 0, 1, 0],
        PatternMeta {
            case_id: "case_001".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: Some("slide_a".into()),
            section_id: Some("section_a".into()),
            stain_batch: Some("batch_a".into()),
            block_id: Some("block_a".into()),
            region_id: Some("region_a".into()),
        },
    )
    .expect("pattern");
    pattern.qc_bin = Some(vec![10, 10, 20, 20].into_boxed_slice());
    pattern.component_id = Some(vec![1, 1, 2, 2].into_boxed_slice());
    pattern.mark_prob = Some(vec![0.90, 0.10, 0.80, 0.20].into_boxed_slice());
    pattern.tumor_probability = Some(vec![0.95, 0.88, 0.76, 0.91].into_boxed_slice());
    pattern.nucleus_area_um2 = Some(vec![42.0, 38.5, 44.5, 39.0].into_boxed_slice());
    pattern.local_dab_od = Some(vec![1.1, 0.2, 1.0, 0.3].into_boxed_slice());
    pattern.local_hematoxylin_od = Some(vec![0.7, 0.8, 0.6, 0.9].into_boxed_slice());

    write_filtered_pattern_export_parquet(&pattern, &path).expect("write parquet");
    let loaded = load_pattern_parquet_with_diagnostics(&path, &mask)
        .expect("load parquet")
        .pattern;

    assert_eq!(loaded.x_um.as_ref(), pattern.x_um.as_ref());
    assert_eq!(loaded.y_um.as_ref(), pattern.y_um.as_ref());
    assert_eq!(loaded.mark.as_ref(), pattern.mark.as_ref());
    assert_eq!(loaded.meta.case_id, "case_001");
    assert_eq!(loaded.meta.timepoint, "post");
    assert_eq!(loaded.meta.protein, "MSH6");
    assert_eq!(loaded.meta.slide_id.as_deref(), Some("slide_a"));
    assert_eq!(loaded.meta.section_id.as_deref(), Some("section_a"));
    assert_eq!(loaded.meta.stain_batch.as_deref(), Some("batch_a"));
    assert_eq!(loaded.meta.block_id.as_deref(), Some("block_a"));
    assert_eq!(loaded.meta.region_id.as_deref(), Some("region_a"));
    assert_eq!(loaded.qc_bin.as_deref(), Some(&[10, 10, 20, 20][..]));
    assert_eq!(loaded.component_id.as_deref(), Some(&[1, 1, 2, 2][..]));
    for field in ["block_id", "slide_region", "stain_batch"] {
        assert!(
            loaded.categorical_strata.contains_key(field),
            "missing {field}"
        );
        assert_eq!(loaded.categorical_strata[field].len(), loaded.len());
    }
    assert_eq!(
        loaded.mark_prob.as_deref(),
        Some(&[0.90, 0.10, 0.80, 0.20][..])
    );
    assert_eq!(
        loaded.tumor_probability.as_deref(),
        Some(&[0.95, 0.88, 0.76, 0.91][..])
    );
    assert_eq!(
        loaded.nucleus_area_um2.as_deref(),
        Some(&[42.0, 38.5, 44.5, 39.0][..])
    );
    assert_eq!(
        loaded.local_dab_od.as_deref(),
        Some(&[1.1, 0.2, 1.0, 0.3][..])
    );
    assert_eq!(
        loaded.local_hematoxylin_od.as_deref(),
        Some(&[0.7, 0.8, 0.6, 0.9][..])
    );
}

#[test]
fn optional_absence_preserved() {
    use std::fs::File;

    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cells.parquet");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");
    let pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1, 0, 1, 0],
        PatternMeta {
            case_id: "case_001".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");

    write_filtered_pattern_export_parquet(&pattern, &path).expect("write parquet");
    let schema = ParquetRecordBatchReaderBuilder::try_new(File::open(&path).expect("open export"))
        .expect("read export metadata")
        .schema()
        .clone();
    assert_eq!(
        schema
            .metadata()
            .get("marklab.export_kind")
            .map(String::as_str),
        Some("filtered_canonical_pattern")
    );
    for absent in [
        "internal_control_local",
        "artifact",
        "edge_artifact",
        "fold_artifact",
        "necrosis",
        "nonviable_therapy_effect",
        "qc_bin",
        "component_id",
    ] {
        assert!(
            schema.field_with_name(absent).is_err(),
            "fabricated {absent}"
        );
    }
    let loaded = load_pattern_parquet_with_diagnostics(&path, &mask)
        .expect("load parquet")
        .pattern;

    assert!(loaded.internal_control_valid_fraction.is_none());
    assert!(loaded.qc_bin.is_none());
    assert!(loaded.component_id.is_none());
}

#[test]
fn parquet_writer_rejects_invalid_tumor_probability_metrics() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cells.parquet");
    let mut pattern = Pattern::from_arrays(
        vec![0.0],
        vec![0.0],
        vec![1],
        PatternMeta {
            case_id: "case_001".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");

    pattern.tumor_probability = Some(vec![1.20].into_boxed_slice());
    let probability_error = write_filtered_pattern_export_parquet(&pattern, &path)
        .expect_err("invalid tumor_probability");
    assert!(probability_error.to_string().contains("tumor_probability"));

    pattern.tumor_probability = Some(vec![0.80].into_boxed_slice());
    pattern.nucleus_area_um2 = Some(vec![-1.0].into_boxed_slice());
    let area_error = write_filtered_pattern_export_parquet(&pattern, &path)
        .expect_err("invalid nucleus_area_um2");
    assert!(area_error.to_string().contains("nucleus_area_um2"));
}

#[test]
fn parquet_loader_rejects_partially_populated_dense_optional_metrics() {
    use std::{fs::File, sync::Arc};

    use arrow::{
        array::{BooleanArray, Float32Array, Float64Array, RecordBatch, StringArray, UInt8Array},
        datatypes::{DataType, Field, Schema},
    };
    use parquet::arrow::arrow_writer::ArrowWriter;

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("partial_metric.parquet");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[2,-1],[2,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");
    let schema = Arc::new(Schema::new(vec![
        Field::new("x_um", DataType::Float64, false),
        Field::new("y_um", DataType::Float64, false),
        Field::new("mark", DataType::UInt8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("timepoint", DataType::Utf8, false),
        Field::new("protein", DataType::Utf8, false),
        Field::new("valid_tumor", DataType::Boolean, false),
        Field::new("valid_ihc", DataType::Boolean, false),
        Field::new("local_dab_od", DataType::Float32, true),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Float64Array::from(vec![0.0, 1.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(UInt8Array::from(vec![1, 0])),
            Arc::new(StringArray::from(vec!["case_001"; 2])),
            Arc::new(StringArray::from(vec!["post"; 2])),
            Arc::new(StringArray::from(vec!["MSH6"; 2])),
            Arc::new(BooleanArray::from(vec![true; 2])),
            Arc::new(BooleanArray::from(vec![true; 2])),
            Arc::new(Float32Array::from(vec![Some(0.25), None])),
        ],
    )
    .expect("batch");
    let file = File::create(&path).expect("file");
    let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");

    let error = load_pattern_parquet_with_diagnostics(&path, &mask)
        .expect_err("partially populated metrics must not receive fabricated values");

    assert!(error.to_string().contains("local_dab_od"));
    assert!(error.to_string().contains("every retained row or none"));
}

#[test]
fn csv_parquet_equivalent_rows_produce_equal_pattern() {
    let dir = tempfile::tempdir().expect("temp dir");
    let csv_path = dir.path().join("cells.csv");
    let parquet_path = dir.path().join("cells.parquet");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");

    fs::write(
        &csv_path,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write csv");

    let pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1, 0, 1, 0],
        PatternMeta {
            case_id: "case_001".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    write_filtered_pattern_export_parquet(&pattern, &parquet_path).expect("write parquet");

    let csv_loaded = load_pattern_path(&csv_path, &mask).expect("load csv by path");
    let parquet_loaded = load_pattern_path(&parquet_path, &mask).expect("load parquet by path");

    assert_eq!(csv_loaded.len(), 4);
    assert_eq!(parquet_loaded.len(), 4);
    assert_eq!(csv_loaded.meta.case_id, "case_001");
    assert_eq!(parquet_loaded.meta.case_id, "case_001");
    assert_eq!(csv_loaded, parquet_loaded);
}

#[test]
fn parquet_loader_uses_internal_control_local_as_validity_mask() {
    use std::{fs::File, sync::Arc};

    use arrow::{
        array::{BooleanArray, Float64Array, RecordBatch, StringArray, UInt8Array},
        datatypes::{DataType, Field, Schema},
    };
    use parquet::arrow::arrow_writer::ArrowWriter;

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cells.parquet");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");
    let schema = Arc::new(Schema::new(vec![
        Field::new("x_um", DataType::Float64, false),
        Field::new("y_um", DataType::Float64, false),
        Field::new("mark", DataType::UInt8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("timepoint", DataType::Utf8, false),
        Field::new("protein", DataType::Utf8, false),
        Field::new("valid_tumor", DataType::Boolean, false),
        Field::new("valid_ihc", DataType::Boolean, false),
        Field::new("internal_control_local", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Float64Array::from(vec![0.0, 1.0, 2.0, 3.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0, 0.0, 0.0])),
            Arc::new(UInt8Array::from(vec![1, 0, 1, 0])),
            Arc::new(StringArray::from(vec!["case_001"; 4])),
            Arc::new(StringArray::from(vec!["post"; 4])),
            Arc::new(StringArray::from(vec!["MSH6"; 4])),
            Arc::new(BooleanArray::from(vec![true, true, false, true])),
            Arc::new(BooleanArray::from(vec![true, true, true, false])),
            Arc::new(StringArray::from(vec![
                Some("valid"),
                Some("valid"),
                Some("absent"),
                Some("unknown"),
            ])),
        ],
    )
    .expect("batch");
    let file = File::create(&path).expect("file");
    let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");

    let loaded = load_pattern_parquet_with_diagnostics(&path, &mask)
        .expect("load parquet")
        .pattern;

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded.n_marked(), 1);
    assert_eq!(loaded.window.valid_mask_fraction, 0.5);
    assert_eq!(loaded.valid_tumor_fraction, Some(0.75));
    assert_eq!(loaded.valid_ihc_fraction, Some(0.75));
    assert_eq!(loaded.internal_control_valid_fraction, Some(0.5));

    #[cfg(feature = "csv")]
    {
        let csv_path = dir.path().join("cells.csv");
        std::fs::write(
            &csv_path,
            "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,internal_control_local\n\
0.0,0.0,1,case_001,post,MSH6,true,true,valid\n\
1.0,0.0,0,case_001,post,MSH6,true,true,valid\n\
2.0,0.0,1,case_001,post,MSH6,false,true,absent\n\
3.0,0.0,0,case_001,post,MSH6,true,false,unknown\n",
        )
        .expect("write CSV parity input");
        let csv_loaded = super::csv::load_pattern_csv_with_diagnostics(&csv_path, &mask)
            .expect("load CSV parity input")
            .pattern;

        assert_eq!(
            (
                csv_loaded.valid_tumor_fraction,
                csv_loaded.valid_ihc_fraction,
                csv_loaded.internal_control_valid_fraction,
                csv_loaded.window.valid_mask_fraction,
            ),
            (
                loaded.valid_tumor_fraction,
                loaded.valid_ihc_fraction,
                loaded.internal_control_valid_fraction,
                loaded.window.valid_mask_fraction,
            )
        );
    }
}

#[test]
fn parquet_loader_excludes_artifact_and_nonviable_rows_from_analysis_window() {
    use std::{fs::File, sync::Arc};

    use arrow::{
        array::{BooleanArray, Float64Array, RecordBatch, StringArray, UInt8Array},
        datatypes::{DataType, Field, Schema},
    };
    use parquet::arrow::arrow_writer::ArrowWriter;

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cells.parquet");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[7,-1],[7,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");
    let schema = Arc::new(Schema::new(vec![
        Field::new("x_um", DataType::Float64, false),
        Field::new("y_um", DataType::Float64, false),
        Field::new("mark", DataType::UInt8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("timepoint", DataType::Utf8, false),
        Field::new("protein", DataType::Utf8, false),
        Field::new("valid_tumor", DataType::Boolean, false),
        Field::new("valid_ihc", DataType::Boolean, false),
        Field::new("artifact", DataType::Boolean, true),
        Field::new("edge_artifact", DataType::Boolean, true),
        Field::new("fold_artifact", DataType::Boolean, true),
        Field::new("necrosis", DataType::Boolean, true),
        Field::new("nonviable_therapy_effect", DataType::Boolean, true),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Float64Array::from(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0])),
            Arc::new(Float64Array::from(vec![0.0; 7])),
            Arc::new(UInt8Array::from(vec![1, 0, 1, 0, 1, 0, 0])),
            Arc::new(StringArray::from(vec!["case_001"; 7])),
            Arc::new(StringArray::from(vec!["post"; 7])),
            Arc::new(StringArray::from(vec!["MSH6"; 7])),
            Arc::new(BooleanArray::from(vec![true; 7])),
            Arc::new(BooleanArray::from(vec![true; 7])),
            Arc::new(BooleanArray::from(vec![
                Some(false),
                Some(true),
                Some(false),
                Some(false),
                Some(false),
                Some(false),
                Some(false),
            ])),
            Arc::new(BooleanArray::from(vec![
                Some(false),
                Some(false),
                Some(true),
                Some(false),
                Some(false),
                Some(false),
                Some(false),
            ])),
            Arc::new(BooleanArray::from(vec![
                Some(false),
                Some(false),
                Some(false),
                Some(true),
                Some(false),
                Some(false),
                Some(false),
            ])),
            Arc::new(BooleanArray::from(vec![
                Some(false),
                Some(false),
                Some(false),
                Some(false),
                Some(true),
                Some(false),
                Some(false),
            ])),
            Arc::new(BooleanArray::from(vec![
                Some(false),
                Some(false),
                Some(false),
                Some(false),
                Some(false),
                Some(true),
                Some(false),
            ])),
        ],
    )
    .expect("batch");
    let file = File::create(&path).expect("file");
    let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");

    let loaded = load_pattern_parquet_with_diagnostics(&path, &mask)
        .expect("load parquet")
        .pattern;

    assert_eq!(loaded.mark.as_ref(), &[1, 0]);
    assert!((loaded.window.valid_mask_fraction - (2.0 / 7.0)).abs() < 1e-12);
    assert_eq!(loaded.valid_tumor_fraction, Some(1.0));
    assert_eq!(loaded.valid_ihc_fraction, Some(1.0));
    assert_eq!(loaded.artifact_excluded_fraction, Some(3.0 / 7.0));
    assert_eq!(loaded.nonviable_excluded_fraction, Some(2.0 / 7.0));
}
