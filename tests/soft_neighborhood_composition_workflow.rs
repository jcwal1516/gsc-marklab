#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use std::collections::BTreeMap;

use marklab::{
    execute_algorithm_with_store, soft_neighborhood_composition, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, DeclaredScalarPatternInput, DurableProject,
    DurableProjectLimits, LocalScheduler, MarkTable, MeasurementStatus, MissingnessPolicy,
    NativeRuntimeProvenance, NodeId, ObservationWindow2D, ObservationWindowLimits,
    ProbabilitySimplexMarkDeclaration, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality,
    ScalarMarkUnit, SchedulerLimits, SoftNeighborhoodCompositionAnalysisNode,
    SoftNeighborhoodCompositionConfig, SoftNeighborhoodCompositionError,
    SoftNeighborhoodCompositionLimits, SoftNeighborhoodCompositionResult, WorkflowGraph,
};
use support::{binary_metadata, fixture, publish_record, MARK_SCHEMA};

const PYTHON_ORACLE: &str = include_str!("fixtures/soft_neighborhood/python_radius_oracle.json");

fn simplex_metadata() -> BTreeMap<String, String> {
    let levels = ["neoplastic", "inflammatory"];
    let digest = marklab::ContentDigest::from_framed(levels.iter().map(|level| level.as_bytes()));
    BTreeMap::from([
        ("levels_count".into(), "2".into()),
        ("levels_digest".into(), digest.to_string()),
        ("mark_id".into(), "cellvit_annotation_probabilities".into()),
        (
            "mark_label".into(),
            "CellViT annotation probabilities".into(),
        ),
        ("measurement_status".into(), "morphology_prediction".into()),
        ("modality".into(), "morphology".into()),
        ("unit".into(), "probability_simplex".into()),
        ("value_kind".into(), "probability_simplex".into()),
    ])
}

fn limits(pair_visits: usize, bytes: usize) -> SoftNeighborhoodCompositionLimits {
    SoftNeighborhoodCompositionLimits::new(4, 2, 8, pair_visits, bytes)
        .expect("neighborhood limits")
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
            b"soft-neighborhood-composition-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    path: &std::path::Path,
    radius: f64,
) -> (CacheStatus, SoftNeighborhoodCompositionResult, usize) {
    let mut fixture = fixture();
    fixture.pattern.x_um = vec![0.0, 1.0, 3.0, 10.0].into_boxed_slice();
    fixture.pattern.y_um = vec![0.0; 4].into_boxed_slice();
    let binary_provenance = publish_record(
        &mut fixture,
        b"soft-neighborhood-binary",
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
    let simplex_provenance = publish_record(
        &mut fixture,
        b"soft-neighborhood-simplex",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        simplex_metadata(),
    );
    let mark_id = ScalarMarkId::new("cellvit_annotation_probabilities").expect("mark ID");
    let table = MarkTable::new(
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
            ScalarMarkColumn::probability_simplex(
                ProbabilitySimplexMarkDeclaration::new(
                    mark_id.clone(),
                    "CellViT annotation probabilities",
                    vec!["neoplastic".into(), "inflammatory".into()],
                    MeasurementStatus::MorphologyPrediction,
                    simplex_provenance,
                )
                .expect("simplex declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::ProbabilitySimplex,
                MissingnessPolicy::NotPermitted,
                vec![
                    vec![1.0, 0.0],
                    vec![0.0, 1.0],
                    vec![0.5, 0.5],
                    vec![0.25, 0.75],
                ],
            )
            .expect("simplex column"),
        ],
        4,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("table");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("input");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[11,-1],[11,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    let config =
        SoftNeighborhoodCompositionConfig::new(radius, limits(16, 1 << 20)).expect("config");
    let node = SoftNeighborhoodCompositionAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("soft-neighborhood-composition").expect("node ID"),
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
        ArtifactSchema::new("marklab.soft_neighborhood_composition", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn complete_simplex_rows_flow_across_one_fixed_physical_radius() {
    let mut fixture = fixture();
    fixture.pattern.x_um = vec![0.0, 1.0, 3.0, 10.0].into_boxed_slice();
    fixture.pattern.y_um = vec![0.0; 4].into_boxed_slice();
    let binary_provenance = publish_record(
        &mut fixture,
        b"soft-neighborhood-binary",
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
    let simplex_provenance = publish_record(
        &mut fixture,
        b"soft-neighborhood-simplex",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        simplex_metadata(),
    );
    let mark_id = ScalarMarkId::new("cellvit_annotation_probabilities").expect("simplex mark ID");
    let table = MarkTable::new(
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
            ScalarMarkColumn::probability_simplex(
                ProbabilitySimplexMarkDeclaration::new(
                    mark_id.clone(),
                    "CellViT annotation probabilities",
                    vec!["neoplastic".into(), "inflammatory".into()],
                    MeasurementStatus::MorphologyPrediction,
                    simplex_provenance,
                )
                .expect("simplex declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::ProbabilitySimplex,
                MissingnessPolicy::NotPermitted,
                vec![
                    vec![1.0, 0.0],
                    vec![0.0, 1.0],
                    vec![0.5, 0.5],
                    vec![0.25, 0.75],
                ],
            )
            .expect("simplex column"),
        ],
        4,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("mark table");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[11,-1],[11,1],[-1,1],[-1,-1]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    let config = SoftNeighborhoodCompositionConfig::new(1.5, limits(16, 1 << 20))
        .expect("neighborhood config");
    let result = soft_neighborhood_composition(&input, &window, &mark_id, &config)
        .expect("soft neighborhoods");
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("Python oracle");
    assert_eq!(result.radius_um, 1.5);
    assert_eq!(
        result.directed_pair_visits,
        oracle["directed_pair_visits"].as_u64().expect("visits") as usize
    );
    assert_eq!(
        result.zero_neighbor_cell_count,
        oracle["zero_neighbor_cell_count"]
            .as_u64()
            .expect("zero neighbors") as usize
    );
    assert_eq!(result.rows.len(), 4);
    assert_eq!(result.rows[0].neighbor_count, 1);
    assert_eq!(
        result.rows[0].mean_neighbor_probabilities,
        Some(vec![0.0, 1.0])
    );
    assert_eq!(
        result.rows[1].mean_neighbor_probabilities,
        Some(vec![1.0, 0.0])
    );
    assert_eq!(result.rows[2].neighbor_count, 0);
    assert_eq!(result.rows[2].mean_neighbor_probabilities, None);
    assert_eq!(
        result.mean_neighbor_class_mass,
        Some(
            oracle["mean_neighbor_class_mass"]
                .as_array()
                .expect("mean mass")
                .iter()
                .map(|value| value.as_f64().expect("mass"))
                .collect()
        )
    );
    let empty = soft_neighborhood_composition(
        &input,
        &window,
        &mark_id,
        &SoftNeighborhoodCompositionConfig::new(0.1, limits(16, 1 << 20))
            .expect("empty graph config"),
    )
    .expect("typed empty graph");
    assert_eq!(empty.zero_neighbor_cell_count, 4);
    assert_eq!(empty.mean_neighbor_class_mass, None);
    assert!(empty
        .rows
        .iter()
        .all(|row| row.mean_neighbor_probabilities.is_none()));
    assert!(matches!(
        soft_neighborhood_composition(
            &input,
            &window,
            &mark_id,
            &SoftNeighborhoodCompositionConfig::new(1.5, limits(1, 1 << 20))
                .expect("pair limit config"),
        ),
        Err(SoftNeighborhoodCompositionError::PairVisitLimitExceeded {
            observed: 2,
            maximum: 1,
        })
    ));
    assert!(matches!(
        soft_neighborhood_composition(
            &input,
            &window,
            &mark_id,
            &SoftNeighborhoodCompositionConfig::new(
                1.5,
                limits(16, result.estimated_storage_bytes - 1),
            )
            .expect("memory limit config"),
        ),
        Err(SoftNeighborhoodCompositionError::RetainedByteLimitExceeded {
            required,
            maximum,
        }) if required == maximum + 1
    ));
}

#[test]
fn invalid_radius_and_one_short_pair_work_fail_explicitly() {
    assert_eq!(
        SoftNeighborhoodCompositionConfig::new(0.0, limits(16, 1 << 20)),
        Err(SoftNeighborhoodCompositionError::InvalidConfig)
    );
}

#[test]
fn soft_neighborhood_reopens_as_a_hit_and_radius_changes_identity() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, 1.5);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&path, 1.5);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&path, 2.1);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
}
