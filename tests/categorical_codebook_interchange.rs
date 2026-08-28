#![cfg(all(feature = "csv", feature = "parquet"))]

use std::{fs, fs::File, sync::Arc};

use arrow::{
    array::{BooleanArray, Float64Array, RecordBatch, StringArray, UInt8Array},
    datatypes::{DataType, Field, Schema},
};
use marklab::{PatternLoader, TumorMask};
use parquet::arrow::arrow_writer::ArrowWriter;

#[test]
fn cellvit_categorical_labels_and_codes_round_trip_csv_to_parquet() {
    let directory = tempfile::tempdir().expect("tempdir");
    let csv = directory.path().join("cells.csv");
    fs::write(
        &csv,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,histologic_compartment\n\
0,0,1,case-1,baseline,cellvit,true,true,Neoplastic\n\
1,0,0,case-1,baseline,cellvit,true,true,Inflammatory\n\
2,0,0,case-1,baseline,cellvit,true,true,Connective\n\
3,0,0,case-1,baseline,cellvit,true,true,Inflammatory\n",
    )
    .expect("CSV fixture");
    let mask = TumorMask::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");
    let loaded = PatternLoader::new(&mask).load(&csv).expect("CSV load");
    assert_eq!(
        loaded.categorical_stratum_levels["histologic_compartment"].as_ref(),
        ["Neoplastic", "Inflammatory", "Connective"]
    );
    assert_eq!(
        loaded.categorical_strata["histologic_compartment"].as_ref(),
        [0, 1, 2, 1]
    );

    let parquet = directory.path().join("cells.parquet");
    let schema = Arc::new(Schema::new(vec![
        Field::new("x_um", DataType::Float64, false),
        Field::new("y_um", DataType::Float64, false),
        Field::new("mark", DataType::UInt8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("timepoint", DataType::Utf8, false),
        Field::new("protein", DataType::Utf8, false),
        Field::new("valid_tumor", DataType::Boolean, false),
        Field::new("valid_ihc", DataType::Boolean, false),
        Field::new("histologic_compartment", DataType::Utf8, false),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Float64Array::from(vec![0.0, 1.0, 2.0, 3.0])),
            Arc::new(Float64Array::from(vec![0.0; 4])),
            Arc::new(UInt8Array::from(vec![1, 0, 0, 0])),
            Arc::new(StringArray::from(vec!["case-1"; 4])),
            Arc::new(StringArray::from(vec!["baseline"; 4])),
            Arc::new(StringArray::from(vec!["cellvit"; 4])),
            Arc::new(BooleanArray::from(vec![true; 4])),
            Arc::new(BooleanArray::from(vec![true; 4])),
            Arc::new(StringArray::from(vec![
                "Neoplastic",
                "Inflammatory",
                "Connective",
                "Inflammatory",
            ])),
        ],
    )
    .expect("Parquet batch");
    let mut writer =
        ArrowWriter::try_new(File::create(&parquet).expect("file"), schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");
    let replay = PatternLoader::new(&mask)
        .load(&parquet)
        .expect("Parquet replay");
    assert_eq!(
        replay.categorical_stratum_levels,
        loaded.categorical_stratum_levels
    );
    assert_eq!(replay.categorical_strata, loaded.categorical_strata);
}

#[test]
#[ignore = "requires the admitted real CellViT coordinate CSV and exact window"]
fn admitted_cellvit_coordinate_csv_retains_its_four_class_codebook() {
    let cells = std::env::var_os("MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV")
        .expect("MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV");
    let window = std::env::var_os("MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW")
        .expect("MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW");
    let header = fs::read_to_string(&cells)
        .expect("real cells")
        .lines()
        .next()
        .expect("header")
        .to_owned();
    assert!(!header.split(',').any(|field| field == "cell_id"));
    let mask = TumorMask::from_geojson_str(&fs::read_to_string(window).expect("real window"))
        .expect("real mask");
    let pattern = PatternLoader::new(&mask)
        .load(cells)
        .expect("real CellViT pattern");
    let levels = &pattern.categorical_stratum_levels["histologic_compartment"];
    let codes = &pattern.categorical_strata["histologic_compartment"];
    let mut counts = std::collections::BTreeMap::new();
    for code in codes {
        *counts
            .entry(levels[*code as usize].as_str())
            .or_insert(0_usize) += 1;
    }
    assert_eq!(
        counts,
        std::collections::BTreeMap::from([
            ("Connective", 118),
            ("Dead", 67),
            ("Inflammatory", 365),
            ("Neoplastic", 1_450),
        ])
    );
}
