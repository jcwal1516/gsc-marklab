#![allow(dead_code)]

use approx::assert_abs_diff_eq;
use marklab::{
    execute_algorithm_with_store, inhomogeneous_categorical_cross_pair_correlation, ArtifactRef,
    ArtifactSchema, BinaryMarkDeclaration, CacheStatus, DeclaredScalarPatternInput, DurableProject,
    DurableProjectLimits, HistologicCompartmentMarkDeclaration,
    InhomogeneousCategoricalCrossPairCorrelationAnalysisNode,
    InhomogeneousCategoricalCrossPairCorrelationConfig, InhomogeneousSpatialConfig,
    InhomogeneousSpatialLimits, LocalScheduler, MarkTable, MeasurementStatus, MissingnessPolicy,
    NativeRuntimeProvenance, NodeId, ObservationWindow2D, ObservationWindowLimits,
    PairCorrelationPointStatus, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
    SchedulerLimits, WorkflowGraph,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

fn input_fixture(codes: [u32; 4]) -> (Fixture, marklab::Pattern, MarkTable, ObservationWindow2D) {
    let mut fixture = fixture();
    let mut pattern = marklab::Pattern::from_arrays(
        vec![9.0, 10.0, 14.0, 15.0],
        vec![10.0, 10.0, 10.0, 10.0],
        vec![0; 4],
        fixture.pattern.meta.clone(),
    )
    .expect("pattern");
    pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        Vec::from(codes).into_boxed_slice(),
    );
    let binary_provenance = publish_record(
        &mut fixture,
        b"inhomogeneous-cross-g-binary-provenance",
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
        b"inhomogeneous-cross-g-category-provenance",
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
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[20,0],[20,20],[0,20],[0,0]]]]}"#,
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

fn config(seed: u64) -> InhomogeneousCategoricalCrossPairCorrelationConfig {
    config_with_bytes(seed, 1 << 20)
}

fn config_with_bytes(
    seed: u64,
    maximum_retained_bytes: usize,
) -> InhomogeneousCategoricalCrossPairCorrelationConfig {
    let intensity = InhomogeneousSpatialConfig::new(
        vec![5.0],
        2.0,
        [20, 20],
        19,
        seed,
        0.05,
        1e-12,
        InhomogeneousSpatialLimits::new(
            16,
            16,
            10_000,
            2_000_000,
            1_000_000,
            1_000_000,
            maximum_retained_bytes,
        )
        .expect("limits"),
    )
    .expect("intensity configuration");
    InhomogeneousCategoricalCrossPairCorrelationConfig::new(intensity, 1.0, "tumor", "stroma")
        .expect("cross-g configuration")
}

fn gaussian_intensity_at_event(event: (f64, f64), other: (f64, f64), bandwidth: f64) -> f64 {
    let mut boundary_mass = 0.0;
    let cell_area = 1.0;
    for y in 0..20 {
        for x in 0..20 {
            let probe = (x as f64 + 0.5, y as f64 + 0.5);
            let dx = event.0 - probe.0;
            let dy = event.1 - probe.1;
            boundary_mass += (-(dx * dx + dy * dy) / (2.0 * bandwidth * bandwidth)).exp()
                / (2.0 * std::f64::consts::PI * bandwidth * bandwidth)
                * cell_area;
        }
    }
    let dx = event.0 - other.0;
    let dy = event.1 - other.1;
    let leave_one_out = (-(dx * dx + dy * dy) / (2.0 * bandwidth * bandwidth)).exp()
        / (2.0 * std::f64::consts::PI * bandwidth * bandwidth);
    2.0 * leave_one_out / boundary_mass
}

#[test]
fn type_specific_leave_one_out_intensities_flow_into_directed_cross_g() {
    let (fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");

    let result =
        inhomogeneous_categorical_cross_pair_correlation(&input, &window, &config(20260829))
            .expect("inhomogeneous categorical cross-g");
    let replay =
        inhomogeneous_categorical_cross_pair_correlation(&input, &window, &config(20260829))
            .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.source_level, "tumor");
    assert_eq!(result.target_level, "stroma");
    assert_eq!(result.source_count, 2);
    assert_eq!(result.target_count, 2);
    assert_eq!(result.source_intensity.point_values.len(), 2);
    assert_eq!(result.target_intensity.point_values.len(), 2);

    let source_intensities = [
        gaussian_intensity_at_event((9.0, 10.0), (10.0, 10.0), 2.0),
        gaussian_intensity_at_event((10.0, 10.0), (9.0, 10.0), 2.0),
    ];
    let target_intensities = [
        gaussian_intensity_at_event((14.0, 10.0), (15.0, 10.0), 2.0),
        gaussian_intensity_at_event((15.0, 10.0), (14.0, 10.0), 2.0),
    ];
    let expected_kernel_sum = 0.75 / (source_intensities[0] * target_intensities[0])
        + 0.75 / (source_intensities[1] * target_intensities[1]);
    let expected_center_sum = 1.0 / source_intensities[0] + 1.0 / source_intensities[1];
    let expected_cross_g =
        expected_kernel_sum / (2.0 * std::f64::consts::PI * 5.0 * expected_center_sum);
    let point = &result.curve[0];
    assert_eq!(point.status, PairCorrelationPointStatus::Available);
    assert_eq!(point.eligible_source_centers, 2);
    assert_eq!(point.directed_source_target_pairs_in_support, 2);
    assert_abs_diff_eq!(
        point.inverse_intensity_kernel_sum,
        expected_kernel_sum,
        epsilon = 1e-9
    );
    assert_abs_diff_eq!(
        point.eligible_source_inverse_intensity_sum,
        expected_center_sum,
        epsilon = 1e-10
    );
    assert_abs_diff_eq!(
        point.cross_g.expect("cross-g value"),
        expected_cross_g,
        epsilon = 1e-12
    );
    assert_eq!(
        result.inference.null_model,
        "independent_fixed_gridded_type_specific_inhomogeneous_binomial"
    );
    assert_eq!(result.inference.simulations_completed, 19);
}

#[test]
fn sparse_type_and_one_short_retained_memory_fail_before_output() {
    let (fixture, pattern, table, window) = input_fixture([0, 0, 0, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    assert!(matches!(
        inhomogeneous_categorical_cross_pair_correlation(&input, &window, &config(7)),
        Err(marklab::InhomogeneousCategoricalCrossPairCorrelationError::SparseLevel {
            level,
            count: 1
        }) if level == "stroma"
    ));

    let (fixture, pattern, table, window) = input_fixture([0, 0, 1, 1]);
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let baseline = inhomogeneous_categorical_cross_pair_correlation(&input, &window, &config(7))
        .expect("baseline");
    assert!(matches!(
        inhomogeneous_categorical_cross_pair_correlation(
            &input,
            &window,
            &config_with_bytes(7, baseline.estimated_storage_bytes - 1)
        ),
        Err(marklab::InhomogeneousCategoricalCrossPairCorrelationError::RetainedByteLimitExceeded {
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
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"inhomogeneous-categorical-cross-g-test",
        )
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
    let node = InhomogeneousCategoricalCrossPairCorrelationAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("inhomogeneous-categorical-cross-g").expect("node ID"),
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
        ArtifactSchema::new(
            "marklab.inhomogeneous_categorical_cross_pair_correlation",
            1,
        )
        .expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, durable.execution_count())
}

#[test]
fn durable_node_reopens_as_a_verified_hit_and_seed_invalidates() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("project");
    assert_eq!(durable_run(&path, 20260829), (CacheStatus::Miss, 1));
    assert_eq!(durable_run(&path, 20260829), (CacheStatus::Hit, 1));
    assert_eq!(durable_run(&path, 20260830), (CacheStatus::Miss, 2));
}
