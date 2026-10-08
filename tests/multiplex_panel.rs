use std::str::FromStr;

use marklab::{
    ArtifactId, AssayMarkDeclaration, AssayMarkValues, CellId, ContentDigest, MarkTable,
    MeasurementStatus, ScalarMarkColumn, ScalarMarkId,
};

fn declaration(name: &str, unit: &str) -> AssayMarkDeclaration {
    AssayMarkDeclaration::new(
        ScalarMarkId::new(name).unwrap(),
        name,
        unit,
        MeasurementStatus::Measured,
        ArtifactId::from_str(&ContentDigest::from_bytes(b"one-panel-source").to_string()).unwrap(),
    )
    .unwrap()
}

fn ids() -> Vec<CellId> {
    (0..4)
        .map(|row| CellId::new(format!("cell-{row}")).unwrap())
        .collect()
}

#[test]
fn one_panel_preserves_multiple_markers_categories_and_missingness_without_a_binary_pattern() {
    let columns = vec![
        ScalarMarkColumn::assay(
            declaration("CD3", "fluorescence_au"),
            AssayMarkValues::Continuous(vec![Some(1.0), None, Some(-2.0), Some(4.0)]),
        )
        .unwrap(),
        ScalarMarkColumn::assay(
            declaration("CD8", "fluorescence_au"),
            AssayMarkValues::Continuous(vec![Some(2.0), Some(3.0), Some(4.0), Some(5.0)]),
        )
        .unwrap(),
        ScalarMarkColumn::assay(
            declaration("CD3_positive", "unitless"),
            AssayMarkValues::Binary(vec![Some(true), None, Some(false), Some(true)]),
        )
        .unwrap(),
        ScalarMarkColumn::assay(
            declaration("CD8_positive", "unitless"),
            AssayMarkValues::Binary(vec![Some(false), Some(true), Some(true), Some(false)]),
        )
        .unwrap(),
        ScalarMarkColumn::assay(
            declaration("cell_type", "categorical"),
            AssayMarkValues::Categorical {
                levels: vec!["immune".into(), "tumor".into()],
                values: vec![Some(0), Some(1), None, Some(0)],
            },
        )
        .unwrap(),
    ];
    let table = MarkTable::new(ids(), columns, 4, 1024).unwrap();
    assert_eq!(table.cell_ids().len(), 4);
    let (metadata, values) = table
        .assay_column(&ScalarMarkId::new("CD3").unwrap())
        .unwrap();
    assert_eq!(metadata.unit(), "fluorescence_au");
    assert_eq!(
        values,
        &AssayMarkValues::Continuous(vec![Some(1.0), None, Some(-2.0), Some(4.0)])
    );
    assert_eq!(
        table.measurement_status(&ScalarMarkId::new("CD8").unwrap()),
        Some(MeasurementStatus::Measured)
    );
    assert!(table
        .assay_column(&ScalarMarkId::new("cell_type").unwrap())
        .is_some());
}

#[test]
fn invalid_assay_values_fail_at_construction() {
    assert!(ScalarMarkColumn::assay(
        declaration("bad", "au"),
        AssayMarkValues::Continuous(vec![Some(f64::NAN)])
    )
    .is_err());
    assert!(ScalarMarkColumn::assay(
        declaration("bad", "um"),
        AssayMarkValues::Binary(vec![Some(true)])
    )
    .is_err());
    assert!(ScalarMarkColumn::assay(
        declaration("bad", "categorical"),
        AssayMarkValues::Categorical {
            levels: vec!["immune".into()],
            values: vec![Some(1)]
        }
    )
    .is_err());
    assert!(ScalarMarkColumn::assay(
        declaration("bad", "categorical"),
        AssayMarkValues::Categorical {
            levels: vec!["immune".into(), "immune".into()],
            values: vec![Some(0)]
        }
    )
    .is_err());
}

#[test]
fn both_spatial_summaries_match_a_direct_graph_oracle_and_keep_unavailable_channels() {
    use marklab::{
        summarize_assay_spatial, AssaySpatialInput, AssaySpatialLimits, AssaySpatialOutcome,
        CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
        GlobalMoranWeightPolicy, ObservationWindow2D, ObservationWindowLimits, SpatialAxis,
    };
    let frame = CoordinateFrameId::new("panel-um").unwrap();
    let declared_frame = CoordinateFrame::new(
        frame.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .unwrap();
    let registry = CoordinateRegistry::new(vec![declared_frame], vec![], vec![], vec![]).unwrap();
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .unwrap()
    .with_coordinate_frame(&registry, frame.clone())
    .unwrap();
    let values = [1.0, 2.0, 4.0, 8.0];
    let columns = vec![
        ScalarMarkColumn::assay(
            declaration("signal", "au"),
            AssayMarkValues::Continuous(values.map(Some).to_vec()),
        )
        .unwrap(),
        ScalarMarkColumn::assay(
            declaration("constant", "au"),
            AssayMarkValues::Continuous(vec![Some(1.0); 4]),
        )
        .unwrap(),
        ScalarMarkColumn::assay(
            declaration("sparse", "au"),
            AssayMarkValues::Continuous(vec![Some(1.0), None, None, Some(4.0)]),
        )
        .unwrap(),
    ];
    let table = MarkTable::new(ids(), columns, 4, 1024).unwrap();
    let selection = ["signal", "constant", "sparse"].map(|id| ScalarMarkId::new(id).unwrap());
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0; 4];
    for policy in [
        GlobalMoranWeightPolicy::BinarySymmetric,
        GlobalMoranWeightPolicy::RowStandardized,
    ] {
        let output = summarize_assay_spatial(
            AssaySpatialInput {
                table: &table,
                x_um: &x,
                y_um: &y,
                coordinate_frame_id: &frame,
            },
            &window,
            &selection,
            1.1,
            policy,
            AssaySpatialLimits {
                maximum_points: 4,
                maximum_directed_edges: 6,
                maximum_channels: 3,
                maximum_edge_evaluations: 24,
            },
        )
        .unwrap();
        let mean = values.iter().sum::<f64>() / 4.0;
        let denominator = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
        let mut numerator_i = 0.0;
        let mut numerator_c = 0.0;
        let mut weight_sum = 0.0;
        for left in 0..4 {
            let neighbors = (0..4)
                .filter(|&right| left != right && (x[left] - x[right]).abs() <= 1.1)
                .collect::<Vec<_>>();
            for right in &neighbors {
                let weight = if policy == GlobalMoranWeightPolicy::RowStandardized {
                    1.0 / neighbors.len() as f64
                } else {
                    1.0
                };
                numerator_i += weight * (values[left] - mean) * (values[*right] - mean);
                numerator_c += weight * (values[left] - values[*right]).powi(2);
                weight_sum += weight;
            }
        }
        let AssaySpatialOutcome::Available { moran_i, geary_c } = output[0].outcome else {
            panic!("available signal")
        };
        assert!((moran_i - 4.0 * numerator_i / (weight_sum * denominator)).abs() < 1e-12);
        assert!((geary_c - 3.0 * numerator_c / (2.0 * weight_sum * denominator)).abs() < 1e-12);
        assert_eq!(output[0].directed_edges, 6);
        assert!(matches!(
            output[1].outcome,
            AssaySpatialOutcome::Unavailable { .. }
        ));
        assert!(matches!(
            output[2].outcome,
            AssaySpatialOutcome::Unavailable { .. }
        ));
        assert_eq!(output[2].missing_rows, 2);
    }
}

#[test]
fn existing_modality_and_unit_enums_remain_exhaustively_matchable() {
    fn modality(value: marklab::ScalarMarkModality) -> u8 {
        match value {
            marklab::ScalarMarkModality::Histology => 0,
            marklab::ScalarMarkModality::Immunohistochemistry => 1,
            marklab::ScalarMarkModality::Morphology => 2,
        }
    }
    fn unit(value: marklab::ScalarMarkUnit) -> u8 {
        match value {
            marklab::ScalarMarkUnit::Categorical => 0,
            marklab::ScalarMarkUnit::Unitless => 1,
            marklab::ScalarMarkUnit::SquareMicrometer => 2,
            marklab::ScalarMarkUnit::ProbabilitySimplex => 3,
            marklab::ScalarMarkUnit::Ordinal => 4,
            marklab::ScalarMarkUnit::EmbeddingVector => 5,
        }
    }
    assert_eq!(modality(marklab::ScalarMarkModality::Histology), 0);
    assert_eq!(unit(marklab::ScalarMarkUnit::Unitless), 1);
}

#[test]
fn observed_row_subgraphs_obey_hand_values_and_exact_work_bounds() {
    use marklab::{
        summarize_assay_spatial, AssaySpatialInput, AssaySpatialLimits, AssaySpatialOutcome,
        CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
        GlobalMoranWeightPolicy, ObservationWindow2D, ObservationWindowLimits, SpatialAxis,
    };
    let frame = CoordinateFrameId::new("observed-um").unwrap();
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .unwrap()],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .unwrap()
    .with_coordinate_frame(&registry, frame.clone())
    .unwrap();
    let table = MarkTable::new(
        ids(),
        vec![ScalarMarkColumn::assay(
            declaration("signal", "au"),
            AssayMarkValues::Continuous(vec![Some(1.0), None, Some(4.0), Some(8.0)]),
        )
        .unwrap()],
        4,
        1024,
    )
    .unwrap();
    let selected = [ScalarMarkId::new("signal").unwrap()];
    let calculate = |edges, work| {
        summarize_assay_spatial(
            AssaySpatialInput {
                table: &table,
                x_um: &[0.0, 1.0, 2.0, 3.0],
                y_um: &[0.0; 4],
                coordinate_frame_id: &frame,
            },
            &window,
            &selected,
            2.1,
            GlobalMoranWeightPolicy::BinarySymmetric,
            AssaySpatialLimits {
                maximum_points: 4,
                maximum_channels: 1,
                maximum_directed_edges: edges,
                maximum_edge_evaluations: work,
            },
        )
    };
    let output = calculate(4, 8).unwrap();
    assert_eq!(output[0].observed_rows, 3);
    assert_eq!(output[0].missing_rows, 1);
    assert_eq!(output[0].directed_edges, 4);
    let AssaySpatialOutcome::Available { moran_i, geary_c } = output[0].outcome else {
        panic!("observed chain")
    };
    assert!((moran_i + 1.0 / 148.0).abs() < 1e-14);
    assert!((geary_c - 75.0 / 148.0).abs() < 1e-14);
    assert!(calculate(3, 8).is_err());
    assert!(calculate(4, 7).is_err());
}
