#![allow(dead_code)]

#[path = "support/declared_scalar.rs"]
mod support;

use marklab::{
    analyze_compartment_interface_profile, execute_algorithm_with_store, ArtifactRef,
    ArtifactSchema, BinaryCompartmentPartition2D, BinaryMarkDeclaration, CacheStatus,
    CompartmentInterfaceAnalysisNode, CompartmentInterfaceError, CompartmentInterfaceLimits,
    CompartmentInterfaceProfile, CompartmentPartitionLimits, DeclaredScalarPatternInput,
    DurableProject, DurableProjectLimits, HistologicCompartmentMarkDeclaration, LocalScheduler,
    MarkTable, MeasurementStatus, MissingnessPolicy, NativeRuntimeProvenance, NodeId,
    ObservationWindow2D, ObservationWindowLimits, ScalarMarkColumn, ScalarMarkId,
    ScalarMarkModality, ScalarMarkUnit, SchedulerLimits, WorkflowGraph,
};
use support::{
    binary_metadata, fixture, histologic_compartment_metadata, publish_record, MARK_SCHEMA,
};

fn partition(fixture: &support::Fixture) -> BinaryCompartmentPartition2D {
    let registry = fixture.project.coordinate_registry().expect("registry");
    let window = |text| {
        ObservationWindow2D::from_geojson_str(text, ObservationWindowLimits::default())
            .expect("window")
            .with_coordinate_frame(registry, fixture.frame_id.clone())
            .expect("framed window")
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

fn input(fixture: &mut support::Fixture, codes: Vec<u32>) -> (MarkTable, ScalarMarkId) {
    fixture.pattern.x_um = vec![2.0, 5.0, 7.0, 8.0].into_boxed_slice();
    fixture.pattern.y_um = vec![5.0; 4].into_boxed_slice();
    fixture.pattern.categorical_strata.insert(
        "histologic_compartment".into(),
        codes.clone().into_boxed_slice(),
    );
    let binary_provenance = publish_record(
        fixture,
        b"interface-binary-provenance",
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
        b"interface-compartment-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        histologic_compartment_metadata(MeasurementStatus::Measured, &["stroma", "tumor"]),
    );
    let binary = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        binary_provenance,
    )
    .expect("binary declaration");
    let compartment = HistologicCompartmentMarkDeclaration::new(
        vec!["stroma".into(), "tumor".into()],
        MeasurementStatus::Measured,
        compartment_provenance,
    )
    .expect("compartment declaration");
    let table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary,
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::histologic_compartment(
                compartment,
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                codes,
            )
            .expect("compartment column"),
        ],
        4,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("mark table");
    (
        table,
        ScalarMarkId::new("histologic_compartment").expect("compartment mark ID"),
    )
}

fn limits(bytes: usize) -> CompartmentInterfaceLimits {
    CompartmentInterfaceLimits::new(4, 4, bytes).expect("interface limits")
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
            b"compartment-interface-profile-test",
        )
        .expect("executable"),
    )
    .expect("runtime")
}

fn run_once(
    project_path: &std::path::Path,
    maximum_queries: usize,
) -> (CacheStatus, CompartmentInterfaceProfile, usize) {
    let mut fixture = fixture();
    let partition = partition(&fixture);
    let (table, _) = input(&mut fixture, vec![0, 0, 1, 1]);
    let declared = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let config =
        CompartmentInterfaceLimits::new(4, maximum_queries, 1 << 20).expect("interface limits");
    let node = CompartmentInterfaceAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("compartment-interface").expect("node ID"),
        &declared,
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
    let mut durable =
        DurableProject::open_or_create(project_path, durable_limits).expect("durable project");
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut fixture.project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.compartment_interface_profile", 1).expect("schema"),
        runtime(),
        &fixture.store,
    )
    .expect("run");
    (run.cache_status, run.output, durable.execution_count())
}

#[test]
fn typed_compartment_rows_flow_to_oriented_cell_interface_profile() {
    let mut fixture = fixture();
    let partition = partition(&fixture);
    let (table, mark_id) = input(&mut fixture, vec![0, 0, 1, 1]);
    let declared = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let result = analyze_compartment_interface_profile(&declared, &partition, &limits(1 << 20))
        .expect("interface profile");
    let replay = analyze_compartment_interface_profile(&declared, &partition, &limits(1 << 20))
        .expect("deterministic replay");
    assert_eq!(result, replay);
    assert_eq!(result.case_id, "declared-case");
    assert_eq!(result.mark_id, mark_id.as_str());
    assert_eq!(result.measurement_status, "measured");
    assert_eq!(result.negative_compartment_id, "stroma");
    assert_eq!(result.positive_compartment_id, "tumor");
    assert_eq!(result.interface_length_um, 10.0);
    assert_eq!(result.query_count, 4);
    assert_eq!(result.rows.len(), 4);
    assert_eq!(result.rows[0].cell_id, "cell-0000");
    assert_eq!(result.rows[0].compartment_id, "stroma");
    assert_eq!(result.rows[0].signed_interface_distance_um, -3.0);
    assert_eq!(
        result.rows[1].signed_interface_distance_um.to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(result.rows[2].signed_interface_distance_um, 2.0);
    assert_eq!(result.rows[3].signed_interface_distance_um, 3.0);
    assert_eq!(result.summaries[0].compartment_id, "stroma");
    assert_eq!(result.summaries[0].cell_count, 2);
    assert_eq!(result.summaries[0].mean_absolute_distance_um, 1.5);
    assert_eq!(result.summaries[1].compartment_id, "tumor");
    assert_eq!(result.summaries[1].cell_count, 2);
    assert_eq!(result.summaries[1].mean_absolute_distance_um, 2.5);
    assert_eq!(result.configuration_digest.len(), 64);
    assert_eq!(result.partition_digest.len(), 64);
}

#[test]
fn spatial_label_mismatch_and_actual_resource_ceilings_fail() {
    let mut mismatch_fixture = fixture();
    let mismatch_partition = partition(&mismatch_fixture);
    let (mismatch_table, _) = input(&mut mismatch_fixture, vec![1, 0, 1, 1]);
    let mismatch_input = DeclaredScalarPatternInput::from_mark_table(
        &mismatch_fixture.project,
        &mismatch_fixture.pattern,
        &mismatch_table,
        mismatch_fixture.slide_id.clone(),
        mismatch_fixture.frame_id.clone(),
    )
    .expect("mismatch input");
    assert!(matches!(
        analyze_compartment_interface_profile(
            &mismatch_input,
            &mismatch_partition,
            &limits(1 << 20),
        ),
        Err(CompartmentInterfaceError::SpatialLabelMismatch { row: 0, .. })
    ));

    let mut fixture = fixture();
    let partition = partition(&fixture);
    let (table, _) = input(&mut fixture, vec![0, 0, 1, 1]);
    let declared = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared input");
    let baseline = analyze_compartment_interface_profile(&declared, &partition, &limits(1 << 20))
        .expect("baseline");
    assert!(matches!(
        analyze_compartment_interface_profile(
            &declared,
            &partition,
            &CompartmentInterfaceLimits::new(3, 4, 1 << 20).expect("point limit"),
        ),
        Err(CompartmentInterfaceError::PointLimitExceeded {
            observed: 4,
            maximum: 3,
        })
    ));
    assert!(matches!(
        analyze_compartment_interface_profile(
            &declared,
            &partition,
            &CompartmentInterfaceLimits::new(4, 3, 1 << 20).expect("query limit"),
        ),
        Err(CompartmentInterfaceError::DistanceQueryLimitExceeded {
            required: 4,
            maximum: 3,
        })
    ));
    assert!(matches!(
        analyze_compartment_interface_profile(
            &declared,
            &partition,
            &limits(baseline.estimated_storage_bytes - 1),
        ),
        Err(CompartmentInterfaceError::RetainedByteLimitExceeded {
            required,
            maximum,
        }) if required == maximum + 1
    ));
}

#[test]
fn compartment_interface_profile_reopens_as_a_verified_hit() {
    let root = tempfile::tempdir().expect("root");
    let project = root.path().join("project");
    let first = run_once(&project, 4);
    assert_eq!(first.0, CacheStatus::Miss);
    assert_eq!(first.2, 1);
    let replay = run_once(&project, 4);
    assert_eq!(replay.0, CacheStatus::Hit);
    assert_eq!(replay.1, first.1);
    assert_eq!(replay.2, 1);
    let changed = run_once(&project, 5);
    assert_eq!(changed.0, CacheStatus::Miss);
    assert_eq!(changed.2, 2);
}
