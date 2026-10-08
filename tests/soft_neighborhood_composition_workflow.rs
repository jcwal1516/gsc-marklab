#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use std::collections::BTreeMap;

use marklab::{
    execute_algorithm_with_store, soft_multiscale_neighborhood_composition,
    soft_neighborhood_composition, ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CacheStatus,
    DeclaredScalarPatternInput, DurableProject, DurableProjectLimits, LocalScheduler, MarkTable,
    MeasurementStatus, MissingnessPolicy, NativeRuntimeProvenance, NodeId, ObservationWindow2D,
    ObservationWindowLimits, ProbabilitySimplexMarkDeclaration, ScalarMarkColumn, ScalarMarkId,
    ScalarMarkModality, ScalarMarkUnit, SchedulerLimits, SoftMultiscaleNeighborhoodAnalysisNode,
    SoftMultiscaleNeighborhoodConfig, SoftMultiscaleNeighborhoodError,
    SoftMultiscaleNeighborhoodLimits, SoftNeighborhoodCompositionAnalysisNode,
    SoftNeighborhoodCompositionConfig, SoftNeighborhoodCompositionError,
    SoftNeighborhoodCompositionLimits, SoftNeighborhoodCompositionResult, WorkflowGraph,
    WorkflowNode,
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
    multiscale_radii: Vec<f64>,
) -> (
    CacheStatus,
    SoftNeighborhoodCompositionResult,
    CacheStatus,
    marklab::SoftMultiscaleNeighborhoodResult,
    usize,
) {
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
    let multiscale_config = SoftMultiscaleNeighborhoodConfig::new(
        multiscale_radii,
        SoftMultiscaleNeighborhoodLimits::new(4, 2, 4, 8, 64, 1 << 20).expect("multiscale limits"),
    )
    .expect("multiscale config");
    let multiscale_node = SoftMultiscaleNeighborhoodAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("soft-multiscale-neighborhood-composition").expect("multiscale node ID"),
        &input,
        &window,
        &mark_id,
        &multiscale_config,
    )
    .expect("multiscale node");
    let graph =
        WorkflowGraph::new([node.spec().clone(), multiscale_node.spec().clone()]).expect("graph");
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
    let multiscale_run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &multiscale_node,
        &scheduler,
        ArtifactSchema::new("marklab.soft_multiscale_neighborhood_composition", 1)
            .expect("multiscale schema"),
        runtime(),
        &fixture.store,
    )
    .expect("multiscale run");
    (
        run.cache_status,
        run.output,
        multiscale_run.cache_status,
        multiscale_run.output,
        durable.execution_count(),
    )
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
    let multiscale_config = SoftMultiscaleNeighborhoodConfig::new(
        vec![1.5, 2.5],
        SoftMultiscaleNeighborhoodLimits::new(4, 2, 2, 8, 32, 1 << 20).expect("multiscale limits"),
    )
    .expect("multiscale config");
    let multiscale =
        soft_multiscale_neighborhood_composition(&input, &window, &mark_id, &multiscale_config)
            .expect("multiscale soft neighborhoods");
    assert_eq!(multiscale.geometry_build_count, 1);
    assert_eq!(multiscale.scales.len(), 2);
    assert_eq!(
        multiscale.total_directed_pair_visits,
        oracle["multiscale"]["total_directed_pair_visits"]
            .as_u64()
            .expect("multiscale visits") as usize
    );
    assert_eq!(multiscale.scales[0].zero_neighbor_cell_count, 2);
    assert_eq!(multiscale.scales[1].directed_pair_visits, 4);
    assert_eq!(multiscale.scales[1].zero_neighbor_cell_count, 1);
    assert_eq!(
        multiscale.scales[1].mean_neighbor_class_mass,
        Some(
            oracle["multiscale"]["scales"][1]["mean_neighbor_class_mass"]
                .as_array()
                .expect("second-scale class mass")
                .iter()
                .map(|value| value.as_f64().expect("class mass"))
                .collect()
        )
    );
    assert_eq!(
        multiscale.adjacent_scale_total_variation_distance,
        vec![Some(
            oracle["multiscale"]["adjacent_scale_total_variation_distance"][0]
                .as_f64()
                .expect("adjacent TV")
        )]
    );
    let empty_multiscale = soft_multiscale_neighborhood_composition(
        &input,
        &window,
        &mark_id,
        &SoftMultiscaleNeighborhoodConfig::new(
            vec![0.1, 0.2],
            SoftMultiscaleNeighborhoodLimits::new(4, 2, 2, 8, 32, 1 << 20)
                .expect("empty multiscale limits"),
        )
        .expect("empty multiscale config"),
    )
    .expect("typed empty multiscale graph");
    assert_eq!(empty_multiscale.total_directed_pair_visits, 0);
    assert_eq!(
        empty_multiscale.adjacent_scale_total_variation_distance,
        vec![None]
    );
    assert!(empty_multiscale
        .scales
        .iter()
        .all(|scale| scale.mean_neighbor_class_mass.is_none()));
    assert!(matches!(
        soft_multiscale_neighborhood_composition(
            &input,
            &window,
            &mark_id,
            &SoftMultiscaleNeighborhoodConfig::new(
                vec![1.5, 2.5],
                SoftMultiscaleNeighborhoodLimits::new(4, 2, 2, 8, 5, 1 << 20)
                    .expect("pair-limited multiscale limits"),
            )
            .expect("pair-limited multiscale config"),
        ),
        Err(SoftMultiscaleNeighborhoodError::PairVisitLimitExceeded {
            observed: 6,
            maximum: 5,
        })
    ));
    assert!(matches!(
        soft_multiscale_neighborhood_composition(
            &input,
            &window,
            &mark_id,
            &SoftMultiscaleNeighborhoodConfig::new(
                vec![1.5, 2.5],
                SoftMultiscaleNeighborhoodLimits::new(
                    4,
                    2,
                    2,
                    8,
                    32,
                    multiscale.estimated_storage_bytes - 1,
                )
                .expect("memory-limited multiscale limits"),
            )
            .expect("memory-limited multiscale config"),
        ),
        Err(SoftMultiscaleNeighborhoodError::RetainedByteLimitExceeded {
            required,
            maximum,
        }) if required == maximum + 1
    ));
    let multiscale_node = SoftMultiscaleNeighborhoodAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("soft-multiscale-validation").expect("validation node ID"),
        &input,
        &window,
        &mark_id,
        &multiscale_config,
    )
    .expect("validation node");
    let mut corrupted = multiscale.clone();
    corrupted.scales[0].rows[0].neighbor_count = 2;
    assert!(multiscale_node.encode_output(&corrupted).is_err());
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
    let multiscale_limits =
        SoftMultiscaleNeighborhoodLimits::new(4, 2, 3, 8, 32, 1 << 20).expect("limits");
    assert_eq!(
        SoftMultiscaleNeighborhoodConfig::new(vec![1.5, 1.5], multiscale_limits),
        Err(SoftMultiscaleNeighborhoodError::InvalidConfig)
    );
    assert_eq!(
        SoftMultiscaleNeighborhoodConfig::new(vec![2.5, 1.5], multiscale_limits),
        Err(SoftMultiscaleNeighborhoodError::InvalidConfig)
    );
}

#[test]
fn soft_neighborhoods_reopen_as_hits_and_radius_changes_identity() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let first = run_once(&path, 1.5, vec![1.5, 2.5]);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, CacheStatus::Miss);
    assert_eq!(first.4, 2);
    let replay = run_once(&path, 1.5, vec![1.5, 2.5]);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, CacheStatus::Hit);
    assert_eq!(replay.3, first.3);
    assert_eq!(replay.4, 2);
    let changed = run_once(&path, 2.1, vec![1.5, 2.6]);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, CacheStatus::Miss);
    assert_eq!(changed.4, 4);
}
