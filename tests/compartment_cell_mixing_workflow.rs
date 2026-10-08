#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use marklab::{
    analyze_compartment_cell_mixing, execute_algorithm_with_store, ArtifactRef, ArtifactSchema,
    BinaryCompartmentPartition2D, BinaryMarkDeclaration, CacheStatus,
    CompartmentCellMixingAnalysisNode, CompartmentCellMixingConfig, CompartmentCellMixingError,
    CompartmentCellMixingLimits, CompartmentCellMixingResult, CompartmentPartitionLimits,
    DeclaredScalarPatternInput, DurableProject, DurableProjectLimits,
    HistologicCompartmentMarkDeclaration, LocalScheduler, MarkTable, MeasurementStatus,
    MissingnessPolicy, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
    SchedulerLimits, WorkflowGraph,
};
use support::{
    binary_metadata, cell_id_text_bytes, fixture, histologic_compartment_metadata, publish_record,
    Fixture, MARK_SCHEMA,
};

const PYTHON_ORACLE: &str =
    include_str!("fixtures/compartment_cell_mixing/python_radius_graph_oracle.json");

fn partition(fixture: &Fixture) -> BinaryCompartmentPartition2D {
    let registry = fixture.project.coordinate_registry().expect("registry");
    let window = |text| {
        ObservationWindow2D::from_geojson_str(text, ObservationWindowLimits::default())
            .expect("window")
            .with_coordinate_frame(registry, fixture.frame_id.clone())
            .expect("bound window")
    };
    BinaryCompartmentPartition2D::new(
        window(
            r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[10,0],[10,10],[5,10],[0,10],[0,0]]]]}"#,
        ),
        "stroma",
        window(
            r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[5,0],[5,10],[0,10],[0,0]]]]}"#,
        ),
        "tumor",
        window(
            r#"{"type":"MultiPolygon","coordinates":[[[[5,0],[10,0],[10,10],[5,10],[5,0]]]]}"#,
        ),
        CompartmentPartitionLimits::new(64).expect("partition limits"),
    )
    .expect("partition")
}

fn table(fixture: &mut Fixture, codes: Vec<u32>) -> MarkTable {
    fixture.pattern.x_um = vec![2.0, 4.0, 6.0, 8.0].into_boxed_slice();
    fixture.pattern.y_um = vec![5.0; 4].into_boxed_slice();
    fixture.pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        codes.clone().into_boxed_slice(),
    );
    let binary_provenance = publish_record(
        fixture,
        b"mixing-binary-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::Measured,
            "independent",
        ),
    );
    let compartment_provenance = publish_record(
        fixture,
        b"mixing-compartment-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(MeasurementStatus::Measured, &["stroma", "tumor"]),
    );
    MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::independent(
                    ScalarMarkId::new("mmr_loss").expect("binary ID"),
                    "MMR loss",
                    MeasurementStatus::Measured,
                    binary_provenance,
                )
                .expect("binary declaration"),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::histologic_compartment(
                HistologicCompartmentMarkDeclaration::new(
                    vec!["stroma".into(), "tumor".into()],
                    MeasurementStatus::Measured,
                    compartment_provenance,
                )
                .expect("compartment declaration"),
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                codes,
            )
            .expect("compartment column"),
        ],
        fixture.cell_ids.len(),
        cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("mark table")
}

fn limits(pair_visits: usize, bytes: usize) -> CompartmentCellMixingLimits {
    CompartmentCellMixingLimits::new(4, 4, pair_visits, bytes).expect("mixing limits")
}

fn config(radius: f64, pair_visits: usize, bytes: usize) -> CompartmentCellMixingConfig {
    CompartmentCellMixingConfig::new(radius, limits(pair_visits, bytes)).expect("mixing config")
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"compartment-cell-mixing-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    path: &std::path::Path,
    radius: f64,
) -> (CacheStatus, CompartmentCellMixingResult, usize) {
    let mut fixture = fixture();
    let partition = partition(&fixture);
    let table = table(&mut fixture, vec![0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let config = config(radius, 64, 1 << 20);
    let node = CompartmentCellMixingAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("compartment-cell-mixing").expect("node ID"),
        &input,
        &partition,
        &config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let durable_limits = DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
        .expect("durable limits");
    let mut durable = DurableProject::open_or_create(path, durable_limits).expect("project");
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.compartment_cell_mixing", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn physical_radius_graph_has_exact_cross_edge_fraction_and_entropy() {
    let mut fixture = fixture();
    let partition = partition(&fixture);
    let table = table(&mut fixture, vec![0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let result = analyze_compartment_cell_mixing(&input, &partition, &config(2.1, 64, 1 << 20))
        .expect("mixing result");
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("Python oracle");
    assert_eq!(result.radius_um, 2.1);
    assert_eq!(result.point_count, 4);
    assert_eq!(
        result.undirected_edge_count,
        oracle["undirected_edge_count"].as_u64().expect("edges") as usize
    );
    assert_eq!(
        result.directed_pair_visits,
        oracle["directed_pair_visits"].as_u64().expect("visits") as usize
    );
    assert_eq!(
        result.cross_compartment_edge_count,
        oracle["cross_compartment_edge_count"]
            .as_u64()
            .expect("cross edges") as usize
    );
    assert_eq!(
        result.cross_compartment_edge_fraction,
        oracle["cross_compartment_edge_fraction"]
            .as_f64()
            .expect("cross fraction")
    );
    assert_eq!(result.random_label_cross_edge_expectation, 2.0 / 3.0);
    assert_eq!(result.cross_edge_fraction_minus_expectation, -1.0 / 3.0);
    let entropy = oracle["edge_type_entropy_nats"]
        .as_f64()
        .expect("edge entropy");
    assert!((result.edge_type_entropy_nats - entropy).abs() < 1e-15);
    assert!((result.normalized_edge_type_entropy - entropy / 2.0_f64.ln()).abs() < 1e-15);
    for summary in [&result.negative, &result.positive] {
        assert_eq!(summary.cell_count, 2);
        assert_eq!(summary.same_compartment_neighbor_incidences, 2);
        assert_eq!(summary.cross_compartment_neighbor_incidences, 1);
        assert!((summary.neighbor_label_entropy_nats - entropy).abs() < 1e-15);
    }
}

#[test]
fn empty_graph_and_actual_work_ceilings_fail_explicitly() {
    let mut fixture = fixture();
    let partition = partition(&fixture);
    let table = table(&mut fixture, vec![0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    assert_eq!(
        analyze_compartment_cell_mixing(&input, &partition, &config(1.9, 64, 1 << 20)),
        Err(CompartmentCellMixingError::NoAdjacencyEdges)
    );
    let baseline = analyze_compartment_cell_mixing(&input, &partition, &config(2.1, 64, 1 << 20))
        .expect("baseline");
    assert!(matches!(
        analyze_compartment_cell_mixing(
            &input,
            &partition,
            &config(2.1, baseline.directed_pair_visits - 1, 1 << 20),
        ),
        Err(CompartmentCellMixingError::PairVisitLimitExceeded { .. })
    ));
    assert!(matches!(
        analyze_compartment_cell_mixing(
            &input,
            &partition,
            &config(2.1, 64, baseline.estimated_storage_bytes - 1),
        ),
        Err(CompartmentCellMixingError::RetainedByteLimitExceeded {
            required,
            maximum,
        }) if required == maximum + 1
    ));
}

#[test]
fn cell_mixing_reopens_as_a_hit_and_radius_changes_identity() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, 2.1);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&path, 2.1);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&path, 2.2);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
}
