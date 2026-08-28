use std::{collections::BTreeMap, fs, path::Path, process::Command};

use marklab::{
    execute_algorithm_with_store, ArtifactDraft, ArtifactId, ArtifactRef, ArtifactSchema,
    BinaryMarkDeclaration, CacheStatus, CategoricalNeighborhoodMixingAnalysisNode,
    CategoricalNeighborhoodMixingConfig, CategoricalNeighborhoodMixingLimits, CohortHierarchy,
    ContentDigest, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, DeclaredScalarPatternInput, DurableProject, DurableProjectLimits, HierarchyId,
    HierarchyNode, HistologicCompartmentMarkDeclaration, LocalArtifactStore, LocalScheduler,
    MarkTable, MarklabProject, MeasurementStatus, MissingnessPolicy, NativeRuntimeProvenance,
    NodeId, ObservationWindow2D, ObservationWindowLimits, PatientId, PatternLoader,
    ProbabilityMarkDeclaration, ProbabilityThresholdComparator, ReplicationRole, ScalarMarkColumn,
    ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, SchedulerLimits, SlideId, SpatialAxis,
    StoreId, TumorMask, WorkflowGraph,
};

const MARK_SCHEMA: &str = "marklab.scalar_mark_provenance";
const THRESHOLD_SCHEMA: &str = "marklab.scalar_threshold_provenance";
const CHILD_PROJECT: &str = "MARKLAB_REAL_CELLVIT_MIXING_CHILD_PROJECT";
const CHILD_RESULT: &str = "MARKLAB_REAL_CELLVIT_MIXING_CHILD_RESULT";
const REAL_CELLS: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_CSV";
const REAL_WINDOW: &str = "MARKLAB_REAL_CELLVIT_CATEGORICAL_WINDOW";

#[test]
#[ignore = "requires the admitted real CellViT coordinate CSV and exact window"]
fn admitted_cellvit_categorical_mixing_replays_across_fresh_processes() {
    let cells = std::env::var_os(REAL_CELLS).expect(REAL_CELLS);
    let window = std::env::var_os(REAL_WINDOW).expect(REAL_WINDOW);
    let root = tempfile::tempdir().expect("temporary real durable project");
    let project = root.path().join("project");
    let executable = std::env::current_exe().expect("current integration-test executable");
    let first_path = root.path().join("first.txt");
    let second_path = root.path().join("second.txt");

    for output in [&first_path, &second_path] {
        let status = Command::new(&executable)
            .arg("--exact")
            .arg("admitted_cellvit_categorical_mixing_child")
            .env(CHILD_PROJECT, &project)
            .env(CHILD_RESULT, output)
            .env(REAL_CELLS, &cells)
            .env(REAL_WINDOW, &window)
            .status()
            .expect("run fresh durable child process");
        assert!(status.success());
    }

    let first = fs::read_to_string(first_path).expect("first receipt");
    let second = fs::read_to_string(second_path).expect("second receipt");
    let first_lines = first.lines().collect::<Vec<_>>();
    let second_lines = second.lines().collect::<Vec<_>>();
    assert_eq!(first_lines[0], "miss");
    assert_eq!(second_lines[0], "hit");
    assert_eq!(first_lines[1], "1");
    assert_eq!(second_lines[1], "1");
    assert_eq!(first_lines[2], second_lines[2]);

    let result: serde_json::Value =
        serde_json::from_str(first_lines[2]).expect("typed result JSON");
    assert_eq!(result["point_count"], 2_000);
    assert_eq!(result["class_ids"].as_array().expect("classes").len(), 4);
    assert!(result["directed_pair_visits"].as_u64().expect("visits") > 0);
}

#[test]
fn admitted_cellvit_categorical_mixing_child() {
    let Some(project) = std::env::var_os(CHILD_PROJECT) else {
        return;
    };
    let output = std::env::var_os(CHILD_RESULT).expect(CHILD_RESULT);
    let cells = std::env::var_os(REAL_CELLS).expect(REAL_CELLS);
    let window = std::env::var_os(REAL_WINDOW).expect(REAL_WINDOW);
    run_real_once(
        Path::new(&project),
        Path::new(&cells),
        Path::new(&window),
        Path::new(&output),
    );
}

fn run_real_once(project_path: &Path, cells_path: &Path, window_path: &Path, output_path: &Path) {
    let mask = TumorMask::from_geojson_str(&fs::read_to_string(window_path).expect("real window"))
        .expect("real mask");
    let pattern = PatternLoader::new(&mask)
        .load(cells_path)
        .expect("real typed Pattern");
    let cell_ids = pattern
        .typed_cell_ids()
        .expect("valid real CellIds")
        .expect("present real CellIds")
        .into_vec();
    let slide_id = SlideId::new(
        pattern
            .meta
            .slide_id
            .as_deref()
            .expect("real owning slide identity"),
    )
    .expect("typed slide ID");
    let patient_id = PatientId::new(&pattern.meta.case_id).expect("typed patient ID");
    let patient = HierarchyId::from(patient_id);
    let slide = HierarchyId::from(slide_id.clone());
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    nodes.extend(cell_ids.iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("real hierarchy");
    let frame_id = CoordinateFrameId::new("cellvit-physical-xy-um").expect("frame ID");
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .expect("physical frame")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("coordinate registry");
    let mut project = MarklabProject::new();
    project
        .install_hierarchy(hierarchy)
        .expect("install hierarchy");
    project
        .install_coordinate_registry(registry)
        .expect("install registry");

    let semantic_root = tempfile::tempdir().expect("semantic store");
    let store = LocalArtifactStore::open(
        semantic_root.path(),
        StoreId::new("real-cellvit-mixing-store").expect("store ID"),
    )
    .expect("store");
    let probability_provenance = publish_record(
        &mut project,
        &store,
        b"real-cellvit-winning-class-confidence",
        MARK_SCHEMA,
        Vec::new(),
        probability_metadata(
            "cellvit_winning_class_confidence",
            MeasurementStatus::MorphologyPrediction,
        ),
    );
    let probability_declaration = ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("cellvit_winning_class_confidence").expect("probability mark ID"),
        MeasurementStatus::MorphologyPrediction,
        probability_provenance,
    )
    .expect("probability declaration");
    let threshold_provenance = publish_record(
        &mut project,
        &store,
        b"real-cellvit-high-confidence-threshold",
        THRESHOLD_SCHEMA,
        vec![probability_provenance],
        threshold_metadata(
            "cellvit_high_confidence",
            probability_declaration.mark_id().as_str(),
            "greater_than_or_equal",
            0.75,
        ),
    );
    let binary_provenance = publish_record(
        &mut project,
        &store,
        b"real-cellvit-high-confidence-binary",
        MARK_SCHEMA,
        vec![probability_provenance, threshold_provenance],
        binary_metadata(
            "cellvit_high_confidence",
            "CellViT high-confidence predicted class",
            MeasurementStatus::MorphologyPrediction,
            "thresholded",
        ),
    );
    let levels = pattern.categorical_stratum_levels["histologic_compartment"].to_vec();
    let level_refs = levels.iter().map(String::as_str).collect::<Vec<_>>();
    let categorical_provenance = publish_record(
        &mut project,
        &store,
        b"real-cellvit-histologic-compartment",
        MARK_SCHEMA,
        Vec::new(),
        histologic_compartment_metadata(MeasurementStatus::MorphologyPrediction, &level_refs),
    );
    let codes = pattern.categorical_strata["histologic_compartment"].to_vec();
    let table = MarkTable::new(
        cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::thresholded(
                    ScalarMarkId::new("cellvit_high_confidence").expect("binary mark ID"),
                    "CellViT high-confidence predicted class",
                    MeasurementStatus::MorphologyPrediction,
                    binary_provenance,
                    probability_declaration.mark_id().clone(),
                    ProbabilityThresholdComparator::GreaterThanOrEqual,
                    0.75,
                    Some(threshold_provenance),
                )
                .expect("binary declaration"),
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::probability(
                probability_declaration,
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                pattern
                    .mark_prob
                    .clone()
                    .expect("winning-class confidences"),
            )
            .expect("probability column"),
            ScalarMarkColumn::histologic_compartment(
                HistologicCompartmentMarkDeclaration::new(
                    levels,
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
        2_000,
        cell_id_text_bytes(&cell_ids),
    )
    .expect("real MarkTable");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &project,
        &pattern,
        &table,
        slide_id,
        frame_id.clone(),
    )
    .expect("real declared input");
    let window = ObservationWindow2D::from_geojson_str(
        &fs::read_to_string(window_path).expect("window bytes"),
        ObservationWindowLimits::default(),
    )
    .expect("observation window")
    .with_coordinate_frame(project.coordinate_registry().expect("registry"), frame_id)
    .expect("framed window");
    let config = CategoricalNeighborhoodMixingConfig::new(
        50.0,
        CategoricalNeighborhoodMixingLimits::new(2_000, 8, 4_000_000, 128 << 20).expect("limits"),
    )
    .expect("config");
    let mark_id = ScalarMarkId::new("histologic_compartment").expect("categorical mark ID");
    let node = CategoricalNeighborhoodMixingAnalysisNode::new(
        &mut project,
        NodeId::new("real-cellvit-categorical-mixing").expect("node ID"),
        &input,
        &window,
        &mark_id,
        &config,
    )
    .expect("analysis node");
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
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_neighborhood_mixing", 1).expect("schema"),
        runtime(),
        &store,
    )
    .expect("durable run");
    let cache = match run.cache_status {
        CacheStatus::Miss => "miss",
        CacheStatus::Hit => "hit",
    };
    let receipt = format!(
        "{cache}\n{}\n{}\n",
        durable.execution_count(),
        serde_json::to_string(&run.output).expect("typed result JSON")
    );
    fs::write(output_path, receipt).expect("child receipt");
}

fn publish_record(
    project: &mut MarklabProject,
    store: &LocalArtifactStore,
    label: &[u8],
    schema_id: &str,
    dependencies: Vec<ArtifactId>,
    metadata: BTreeMap<String, String>,
) -> ArtifactId {
    let draft = ArtifactDraft::new(
        ArtifactRef::from_bytes("application/json", label).expect("content ref"),
        ArtifactSchema::new(schema_id, 1).expect("schema"),
        None,
        dependencies,
        metadata,
    )
    .expect("artifact draft");
    let record = store
        .publish_new_send(&draft, |writer| writer.write_all(label))
        .expect("publish provenance")
        .into_record();
    let id = record.id();
    project
        .register_artifact(record)
        .expect("register provenance");
    id
}

fn binary_metadata(
    mark_id: &str,
    label: &str,
    status: MeasurementStatus,
    origin: &str,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), mark_id.into()),
        ("mark_label".into(), label.into()),
        (
            "measurement_status".into(),
            measurement_status_name(status).into(),
        ),
        ("origin".into(), origin.into()),
        ("unit".into(), "unitless".into()),
        ("value_kind".into(), "binary".into()),
    ])
}

fn histologic_compartment_metadata(
    status: MeasurementStatus,
    levels: &[&str],
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "levels_digest".into(),
            ContentDigest::from_framed(levels.iter().map(|level| level.as_bytes())).to_string(),
        ),
        ("levels_count".into(), levels.len().to_string()),
        ("mark_id".into(), "histologic_compartment".into()),
        ("mark_label".into(), "Histologic compartment".into()),
        (
            "measurement_status".into(),
            measurement_status_name(status).into(),
        ),
        ("modality".into(), "histology".into()),
        ("unit".into(), "categorical".into()),
        ("value_kind".into(), "categorical".into()),
    ])
}

fn probability_metadata(mark_id: &str, status: MeasurementStatus) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), mark_id.into()),
        (
            "measurement_status".into(),
            measurement_status_name(status).into(),
        ),
        ("unit".into(), "unitless".into()),
        ("value_kind".into(), "probability".into()),
    ])
}

fn threshold_metadata(
    binary_mark_id: &str,
    probability_mark_id: &str,
    comparator: &str,
    threshold: f32,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("binary_mark_id".into(), binary_mark_id.into()),
        ("comparator".into(), comparator.into()),
        ("probability_mark_id".into(), probability_mark_id.into()),
        (
            "threshold_f32_bits".into(),
            format!("{:08x}", threshold.to_bits()),
        ),
        ("unit".into(), "probability".into()),
    ])
}

fn measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

fn cell_id_text_bytes(cell_ids: &[marklab::CellId]) -> usize {
    cell_ids.iter().map(|id| id.as_str().len()).sum()
}

fn runtime() -> NativeRuntimeProvenance {
    NativeRuntimeProvenance::new(
        "0.1.0-real-cellvit",
        None,
        None,
        "rustc 1.96.0",
        vec!["real-cellvit-categorical".into()],
        ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"categorical-neighborhood-mixing-real-cellvit-v1",
        )
        .expect("executable"),
    )
    .expect("runtime")
}
