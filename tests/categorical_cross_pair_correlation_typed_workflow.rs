#![allow(dead_code)]

use approx::assert_abs_diff_eq;
use marklab::{
    categorical_cross_pair_correlation, execute_algorithm_with_store,
    isotropic_categorical_cross_pair_correlation, translation_categorical_cross_pair_correlation,
    ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CacheStatus,
    CategoricalCrossPairCorrelationAnalysisNode, CategoricalCrossPairCorrelationConfig,
    CategoricalPairLimits, DeclaredScalarPatternInput, DurableProject, DurableProjectLimits,
    HistologicCompartmentMarkDeclaration, IsotropicCategoricalCrossPairCorrelationAnalysisNode,
    IsotropicCategoricalCrossPairCorrelationConfig, IsotropicSpatialLimits, LocalScheduler,
    MarkTable, MeasurementStatus, MissingnessPolicy, NativeRuntimeProvenance, NodeId,
    ObservationWindow2D, ObservationWindowLimits, PairCorrelationKernel,
    PairCorrelationPointStatus, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
    SchedulerLimits, TranslationCategoricalCrossPairCorrelationAnalysisNode,
    TranslationCategoricalCrossPairCorrelationConfig, TranslationSpatialLimits, WorkflowGraph,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

const PYTHON_ORACLE: &str =
    include_str!("fixtures/categorical_cross_pair_correlation/python_line_oracle.json");

fn input_fixture(codes: [u32; 4]) -> (Fixture, marklab::Pattern, MarkTable, ObservationWindow2D) {
    let mut fixture = fixture();
    let mut pattern = fixture.pattern.clone();
    pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        Vec::from(codes).into_boxed_slice(),
    );
    let binary_provenance = publish_record(
        &mut fixture,
        b"cross-g-binary-provenance",
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
    let category_provenance = publish_record(
        &mut fixture,
        b"cross-g-category-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(
            MeasurementStatus::ImportedPrediction,
            &["tumor", "stroma"],
        ),
    );
    let table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::independent(
                    ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
                    "MMR loss",
                    MeasurementStatus::Measured,
                    binary_provenance,
                )
                .expect("binary declaration"),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::histologic_compartment(
                HistologicCompartmentMarkDeclaration::new(
                    vec!["tumor".into(), "stroma".into()],
                    MeasurementStatus::ImportedPrediction,
                    category_provenance,
                )
                .expect("category declaration"),
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                pattern.categorical_strata["histologic_compartment"].clone(),
            )
            .expect("category column"),
        ],
        fixture.cell_ids.len(),
        cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("mark table");
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[-2,-2],[5,-2],[5,2],[-2,2],[-2,-2]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    (fixture, pattern, table, window)
}

fn config(seed: u64) -> CategoricalCrossPairCorrelationConfig {
    config_with_bytes(seed, 1 << 20)
}

fn config_with_bytes(
    seed: u64,
    maximum_retained_bytes: usize,
) -> CategoricalCrossPairCorrelationConfig {
    CategoricalCrossPairCorrelationConfig::new(
        vec![1.0],
        0.5,
        "tumor",
        "stroma",
        31,
        seed,
        0.05,
        CategoricalPairLimits::new(16, 16, 64, 64 * 31, maximum_retained_bytes).expect("limits"),
    )
    .expect("configuration")
}

#[test]
fn directed_epanechnikov_cross_g_matches_the_hand_oracle() {
    let (fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");

    let result = categorical_cross_pair_correlation(&input, &window, &config(20260827))
        .expect("cross-g result");
    let replay = categorical_cross_pair_correlation(&input, &window, &config(20260827))
        .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.kernel, PairCorrelationKernel::Epanechnikov);
    assert_eq!(result.source_level, "tumor");
    assert_eq!(result.target_level, "stroma");
    assert_eq!(result.source_count, 2);
    assert_eq!(result.target_count, 2);
    assert_eq!(result.measurement_status, "imported_prediction");
    assert_eq!(result.configuration_digest.len(), 64);

    let point = &result.curve[0];
    assert_eq!(point.status, PairCorrelationPointStatus::Available);
    assert_eq!(point.eligible_source_centers, 2);
    assert_eq!(point.directed_source_target_pairs_in_support, 1);
    assert_abs_diff_eq!(point.kernel_weight_sum, 1.5, epsilon = 1e-12);
    assert_abs_diff_eq!(
        point.cross_g.expect("cross g"),
        28.0 * 1.5 / (2.0 * std::f64::consts::PI * 1.0 * 2.0 * 2.0),
        epsilon = 1e-12
    );
    assert_eq!(point.theoretical_cross_g, 1.0);
    assert_eq!(result.inference.permutations_completed, 31);
}

#[test]
fn direct_python_pair_loop_and_directional_control_agree() {
    let oracle: serde_json::Value = serde_json::from_str(PYTHON_ORACLE).expect("oracle JSON");
    let run = |codes| {
        let (fixture, pattern, table, window) = input_fixture(codes);
        let input = DeclaredScalarPatternInput::from_mark_table(
            &fixture.project,
            &pattern,
            &table,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
        )
        .expect("declared input");
        categorical_cross_pair_correlation(&input, &window, &config(59)).expect("cross-g result")
    };
    let segregated = run([0, 0, 1, 1]);
    let alternating = run([0, 1, 0, 1]);
    let point = &segregated.curve[0];
    assert_eq!(
        point.eligible_source_centers,
        oracle["eligible_source_centers"].as_u64().expect("centers") as usize
    );
    assert_eq!(
        point.directed_source_target_pairs_in_support,
        oracle["directed_source_target_pairs_in_support"]
            .as_u64()
            .expect("pairs") as usize
    );
    assert_abs_diff_eq!(
        point.kernel_weight_sum,
        oracle["kernel_weight_sum"].as_f64().expect("weight"),
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.cross_g.expect("cross g"),
        oracle["cross_g"].as_f64().expect("oracle cross g"),
        epsilon = 1e-12
    );
    assert!(
        alternating.curve[0].cross_g.expect("alternating cross g")
            > segregated.curve[0].cross_g.expect("segregated cross g")
    );
}

#[test]
fn invalid_support_unknown_levels_and_one_short_memory_fail() {
    let limits = CategoricalPairLimits::new(16, 16, 64, 64 * 31, 1 << 20).expect("limits");
    assert!(CategoricalCrossPairCorrelationConfig::new(
        vec![0.5],
        0.5,
        "tumor",
        "stroma",
        31,
        7,
        0.05,
        limits
    )
    .is_err());
    assert!(CategoricalCrossPairCorrelationConfig::new(
        vec![1.0],
        0.5,
        "tumor",
        "tumor",
        31,
        7,
        0.05,
        limits
    )
    .is_err());

    let (fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let unknown = CategoricalCrossPairCorrelationConfig::new(
        vec![1.0],
        0.5,
        "immune",
        "stroma",
        31,
        7,
        0.05,
        limits,
    )
    .expect("unknown-level config");
    assert!(matches!(
        categorical_cross_pair_correlation(&input, &window, &unknown),
        Err(marklab::CategoricalCrossPairCorrelationError::MissingLevel(level))
            if level == "immune"
    ));

    let baseline =
        categorical_cross_pair_correlation(&input, &window, &config(7)).expect("baseline result");
    let one_short = config_with_bytes(7, baseline.estimated_storage_bytes - 1);
    assert!(matches!(
        categorical_cross_pair_correlation(&input, &window, &one_short),
        Err(marklab::CategoricalCrossPairCorrelationError::RetainedByteLimitExceeded {
            required,
            maximum
        }) if required == maximum + 1
    ));
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.0.0-test",
        None,
        None,
        "rustc 1.96.0-test",
        vec!["test".into()],
        ArtifactRef::from_bytes("application/vnd.marklab.executable", b"cross-g-test")
            .expect("executable"),
    )
    .expect("runtime")
}

fn durable_run(path: &std::path::Path, seed: u64) -> (CacheStatus, usize) {
    let (mut fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let config = config(seed);
    let node = CategoricalCrossPairCorrelationAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("categorical-cross-g").expect("node ID"),
        &input,
        &window,
        &config,
    )
    .expect("node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 1 << 20,
    })
    .expect("scheduler");
    let limits = DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20)
        .expect("durable limits");
    let mut durable = DurableProject::open_or_create(path, limits).expect("durable project");
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_cross_pair_correlation", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, durable.execution_count())
}

#[test]
fn categorical_cross_g_reopens_as_a_store_verified_hit() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    assert_eq!(durable_run(&path, 20260827), (CacheStatus::Miss, 1));
    assert_eq!(durable_run(&path, 20260827), (CacheStatus::Hit, 1));
    assert_eq!(durable_run(&path, 20260828), (CacheStatus::Miss, 2));
}

#[test]
fn translation_corrected_directed_cross_g_matches_the_rectangle_overlap_oracle() {
    let (fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let config = TranslationCategoricalCrossPairCorrelationConfig::new(
        config(20260829),
        TranslationSpatialLimits::new(16, 16, 64, 64, 2_048, 64, 1, 1 << 20)
            .expect("translation limits"),
    )
    .expect("translation cross-g config");

    let result = translation_categorical_cross_pair_correlation(&input, &window, &config)
        .expect("translation cross-g");
    let point = &result.curve[0];
    assert_eq!(result.edge_correction, "translation");
    assert_eq!(point.directed_source_target_pairs_in_support, 1);
    assert_eq!(point.overlap_evaluations, 1);
    assert_abs_diff_eq!(
        point.translation_overlap_area_sum_um2,
        24.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.translation_weighted_kernel_sum,
        7.0 / 4.0,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.cross_g.expect("cross g"),
        49.0 / (8.0 * std::f64::consts::PI),
        epsilon = 1e-12
    );
}

fn translation_config(seed: u64) -> TranslationCategoricalCrossPairCorrelationConfig {
    TranslationCategoricalCrossPairCorrelationConfig::new(
        config(seed),
        TranslationSpatialLimits::new(16, 16, 64, 64, 2_048, 64, 1, 1 << 20)
            .expect("translation limits"),
    )
    .expect("translation config")
}

#[test]
fn translation_cross_g_reopens_as_a_store_verified_hit() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let run_once = |seed| {
        let (mut fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
        let input = DeclaredScalarPatternInput::from_mark_table(
            &fixture.project,
            &pattern,
            &table,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
        )
        .expect("input");
        let config = translation_config(seed);
        let node = TranslationCategoricalCrossPairCorrelationAnalysisNode::new(
            &mut fixture.project,
            NodeId::new("translation-categorical-cross-g").expect("ID"),
            &input,
            &window,
            &config,
        )
        .expect("node");
        let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
        let scheduler = LocalScheduler::new(SchedulerLimits {
            max_inline_output_bytes: 1 << 20,
        })
        .expect("scheduler");
        let mut durable = DurableProject::open_or_create(
            &path,
            DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20).expect("limits"),
        )
        .expect("durable");
        let result = execute_algorithm_with_store(
            &mut durable,
            &mut fixture.project,
            &graph,
            &node,
            &scheduler,
            ArtifactSchema::new("marklab.translation_categorical_cross_g", 1).expect("schema"),
            runtime(),
            &fixture.store,
        )
        .expect("run");
        (result.cache_status, durable.execution_count())
    };
    assert_eq!(run_once(20260829), (CacheStatus::Miss, 1));
    assert_eq!(run_once(20260829), (CacheStatus::Hit, 1));
    assert_eq!(run_once(20260830), (CacheStatus::Miss, 2));
}

#[test]
fn isotropic_directed_cross_g_uses_the_source_centered_visible_fraction() {
    let (fixture, pattern, table, _) = input_fixture([0, 1, 1, 0]);
    let window = ObservationWindow2D::from_geojson_str(
        r#"{"type":"MultiPolygon","coordinates":[[[[0,-2],[4,-2],[4,2],[0,2],[0,-2]]]]}"#,
        ObservationWindowLimits::default(),
    )
    .expect("window")
    .with_coordinate_frame(
        fixture.project.coordinate_registry().expect("registry"),
        fixture.frame_id.clone(),
    )
    .expect("framed window");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let config = IsotropicCategoricalCrossPairCorrelationConfig::new(
        config(20260829),
        IsotropicSpatialLimits::new(16, 16, 64, 128, 512, 1_024, 1, 1 << 20)
            .expect("isotropic limits"),
    )
    .expect("isotropic cross-g config");

    let result = isotropic_categorical_cross_pair_correlation(&input, &window, &config)
        .expect("isotropic cross-g");
    let point = &result.curve[0];
    assert_eq!(result.edge_correction, "isotropic");
    assert_eq!(point.directed_source_target_pairs_in_support, 2);
    assert_eq!(point.visible_arc_evaluations, 2);
    assert_abs_diff_eq!(point.visible_arc_fraction_sum, 1.5, epsilon = 1e-12);
    assert_abs_diff_eq!(
        point.inverse_visible_arc_weighted_kernel_sum,
        4.5,
        epsilon = 1e-12
    );
    assert_abs_diff_eq!(
        point.cross_g.expect("cross g"),
        9.0 / std::f64::consts::PI,
        epsilon = 1e-12
    );
}

fn isotropic_config(seed: u64) -> IsotropicCategoricalCrossPairCorrelationConfig {
    IsotropicCategoricalCrossPairCorrelationConfig::new(
        config(seed),
        IsotropicSpatialLimits::new(16, 16, 64, 128, 512, 1_024, 1, 1 << 20)
            .expect("isotropic limits"),
    )
    .expect("isotropic config")
}

#[test]
fn isotropic_cross_g_reopens_as_a_store_verified_hit() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    let run_once = |seed| {
        let (mut fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
        let input = DeclaredScalarPatternInput::from_mark_table(
            &fixture.project,
            &pattern,
            &table,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
        )
        .expect("input");
        let config = isotropic_config(seed);
        let node = IsotropicCategoricalCrossPairCorrelationAnalysisNode::new(
            &mut fixture.project,
            NodeId::new("isotropic-categorical-cross-g").expect("ID"),
            &input,
            &window,
            &config,
        )
        .expect("node");
        let graph = WorkflowGraph::new([node.spec().clone()]).expect("graph");
        let scheduler = LocalScheduler::new(SchedulerLimits {
            max_inline_output_bytes: 1 << 20,
        })
        .expect("scheduler");
        let mut durable = DurableProject::open_or_create(
            &path,
            DurableProjectLimits::new(64 * 1024, 1 << 20, 64, 64 * 1024, 1 << 20).expect("limits"),
        )
        .expect("durable");
        let result = execute_algorithm_with_store(
            &mut durable,
            &mut fixture.project,
            &graph,
            &node,
            &scheduler,
            ArtifactSchema::new("marklab.isotropic_categorical_cross_g", 1).expect("schema"),
            runtime(),
            &fixture.store,
        )
        .expect("run");
        (result.cache_status, durable.execution_count())
    };
    assert_eq!(run_once(20260829), (CacheStatus::Miss, 1));
    assert_eq!(run_once(20260829), (CacheStatus::Hit, 1));
    assert_eq!(run_once(20260830), (CacheStatus::Miss, 2));
}
