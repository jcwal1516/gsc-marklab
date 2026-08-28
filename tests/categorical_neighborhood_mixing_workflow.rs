#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use marklab::{
    categorical_neighborhood_mixing, execute_algorithm_with_store, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, CategoricalNeighborhoodMixingAnalysisNode,
    CategoricalNeighborhoodMixingConfig, CategoricalNeighborhoodMixingError,
    CategoricalNeighborhoodMixingLimits, CategoricalNeighborhoodMixingResult,
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
    include_str!("fixtures/categorical_neighborhood_mixing/python_oracle.json");

fn table(fixture: &mut Fixture, codes: Vec<u32>) -> MarkTable {
    fixture.pattern.x_um = vec![0.0, 1.0, 2.5, 4.0].into_boxed_slice();
    fixture.pattern.y_um = vec![0.0; 4].into_boxed_slice();
    fixture.pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        codes.clone().into_boxed_slice(),
    );
    fixture.pattern.categorical_stratum_levels.insert(
        "histologic_compartment".into(),
        vec![
            "neoplastic".into(),
            "inflammatory".into(),
            "connective".into(),
        ]
        .into_boxed_slice(),
    );
    let binary_provenance = publish_record(
        fixture,
        b"categorical-mixing-binary",
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
    let categorical_provenance = publish_record(
        fixture,
        b"categorical-mixing-cellvit",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(
            MeasurementStatus::MorphologyPrediction,
            &["neoplastic", "inflammatory", "connective"],
        ),
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
                    vec![
                        "neoplastic".into(),
                        "inflammatory".into(),
                        "connective".into(),
                    ],
                    MeasurementStatus::MorphologyPrediction,
                    categorical_provenance,
                )
                .expect("categorical declaration"),
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                codes,
            )
            .expect("categorical column"),
        ],
        fixture.cell_ids.len(),
        cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("mark table")
}

fn window(fixture: &Fixture) -> ObservationWindow2D {
    ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[5,-1],[5,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window")
}

fn limits(pair_visits: usize, bytes: usize) -> CategoricalNeighborhoodMixingLimits {
    CategoricalNeighborhoodMixingLimits::new(4, 3, pair_visits, bytes).expect("limits")
}

fn config(radius: f64, pair_visits: usize, bytes: usize) -> CategoricalNeighborhoodMixingConfig {
    CategoricalNeighborhoodMixingConfig::new(radius, limits(pair_visits, bytes)).expect("config")
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
            b"categorical-neighborhood-mixing-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    path: &std::path::Path,
    radius: f64,
) -> (CacheStatus, CategoricalNeighborhoodMixingResult, usize) {
    let mut fixture = fixture();
    let table = table(&mut fixture, vec![0, 1, 1, 2]);
    let window = window(&fixture);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("input");
    let mark_id = ScalarMarkId::new("histologic_compartment").expect("mark ID");
    let config = config(radius, 64, 1 << 20);
    let node = CategoricalNeighborhoodMixingAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("categorical-neighborhood-mixing").expect("node ID"),
        &input,
        &window,
        &mark_id,
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
        ArtifactSchema::new("marklab.categorical_neighborhood_mixing", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn three_class_cellvit_codes_produce_the_exact_mixing_matrix() {
    let mut fixture = fixture();
    let table = table(&mut fixture, vec![0, 1, 1, 2]);
    let window = window(&fixture);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("input");
    let mark_id = ScalarMarkId::new("histologic_compartment").expect("mark ID");
    let result =
        categorical_neighborhood_mixing(&input, &window, &mark_id, &config(1.6, 64, 1 << 20))
            .expect("mixing");
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("Python oracle");
    assert_eq!(
        result.class_ids,
        ["neoplastic", "inflammatory", "connective"]
    );
    assert_eq!(result.cell_counts, [1, 2, 1]);
    assert_eq!(result.undirected_edge_count, 3);
    assert_eq!(result.directed_pair_visits, 6);
    assert_eq!(result.directed_pair_counts, [0, 1, 0, 1, 2, 1, 0, 1, 0]);
    assert_eq!(result.cross_class_edge_count, 2);
    assert_eq!(result.cross_class_edge_fraction, 2.0 / 3.0);
    assert_eq!(result.random_label_cross_edge_expectation, 5.0 / 6.0);
    assert!((result.cross_edge_fraction_minus_expectation + 1.0 / 6.0).abs() < 1e-15);
    assert_eq!(result.class_summaries[1].neighbor_incidences, [1, 2, 1]);
    assert_eq!(result.zero_neighbor_cell_count, 0);
    assert_eq!(
        result.directed_pair_counts,
        oracle["directed_pair_counts"]
            .as_array()
            .expect("matrix")
            .iter()
            .map(|value| value.as_u64().expect("count") as usize)
            .collect::<Vec<_>>()
    );
    for (actual, expected) in result.observed_pair_fractions.iter().zip(
        oracle["observed_pair_fractions"]
            .as_array()
            .expect("fractions"),
    ) {
        assert_close(*actual, expected.as_f64().expect("fraction"));
    }
    for (actual, expected) in result.random_label_pair_expectations.iter().zip(
        oracle["random_label_pair_expectations"]
            .as_array()
            .expect("expectations"),
    ) {
        assert_close(*actual, expected.as_f64().expect("expectation"));
    }
    for (actual, expected) in result
        .pair_fraction_excess
        .iter()
        .zip(oracle["pair_fraction_excess"].as_array().expect("excess"))
    {
        assert_close(*actual, expected.as_f64().expect("excess value"));
    }
    assert_close(
        result.class_summaries[1]
            .neighbor_label_entropy_nats
            .expect("entropy"),
        oracle["class_summaries"][1]["neighbor_label_entropy_nats"]
            .as_f64()
            .expect("oracle entropy"),
    );
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-15, "{actual} != {expected}");
}

#[test]
fn graph_and_resource_boundaries_fail_explicitly() {
    let mut fixture = fixture();
    let table = table(&mut fixture, vec![0, 1, 1, 2]);
    let window = window(&fixture);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("input");
    let mark_id = ScalarMarkId::new("histologic_compartment").expect("mark ID");
    assert_eq!(
        categorical_neighborhood_mixing(&input, &window, &mark_id, &config(0.9, 64, 1 << 20)),
        Err(CategoricalNeighborhoodMixingError::NoAdjacencyEdges)
    );
    let baseline =
        categorical_neighborhood_mixing(&input, &window, &mark_id, &config(1.6, 64, 1 << 20))
            .expect("baseline");
    assert!(matches!(
        categorical_neighborhood_mixing(
            &input,
            &window,
            &mark_id,
            &config(1.6, baseline.directed_pair_visits - 1, 1 << 20),
        ),
        Err(CategoricalNeighborhoodMixingError::PairVisitLimitExceeded { .. })
    ));
    assert!(matches!(
        categorical_neighborhood_mixing(
            &input,
            &window,
            &mark_id,
            &config(1.6, 64, baseline.estimated_storage_bytes - 1),
        ),
        Err(CategoricalNeighborhoodMixingError::RetainedByteLimitExceeded {
            required,
            maximum,
        }) if required == maximum + 1
    ));
}

#[test]
fn typed_pattern_codebook_must_match_the_mark_table_declaration() {
    let mut fixture = fixture();
    let table = table(&mut fixture, vec![0, 1, 1, 2]);
    fixture.pattern.categorical_stratum_levels.insert(
        "histologic_compartment".into(),
        vec![
            "inflammatory".into(),
            "neoplastic".into(),
            "connective".into(),
        ]
        .into_boxed_slice(),
    );
    assert!(matches!(
        DeclaredScalarPatternInput::from_mark_table(
            &fixture.project,
            &fixture.pattern,
            &table,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
        ),
        Err(marklab::DeclaredScalarInputError::CategoricalDeclarationMismatch)
    ));
}

#[test]
fn categorical_mixing_reopens_as_a_hit_without_a_second_execution() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, 1.6);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&path, 1.6);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&path, 1.7);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
}
