use std::{collections::BTreeMap, fs, path::Path};

use marklab::{
    ArtifactDraft, ArtifactId, ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CohortHierarchy,
    ContentDigest, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, HierarchyId, HierarchyNode, HistologicCompartmentMarkDeclaration,
    LocalArtifactStore, MarkTable, MarklabProject, MeasurementStatus, MissingnessPolicy,
    ObservationWindow2D, ObservationWindowLimits, PatientId, Pattern, PatternLoader,
    ProbabilityMarkDeclaration, ProbabilityThresholdComparator, ReplicationRole, ScalarMarkColumn,
    ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, SlideId, SpatialAxis, StoreId, TumorMask,
};

const MARK_SCHEMA: &str = "marklab.scalar_mark_provenance";
const THRESHOLD_SCHEMA: &str = "marklab.scalar_threshold_provenance";

pub(crate) struct RealCategoricalFixture {
    #[allow(dead_code)]
    semantic_root: tempfile::TempDir,
    pub(crate) store: LocalArtifactStore,
    pub(crate) project: MarklabProject,
    pub(crate) pattern: Pattern,
    pub(crate) slide_id: SlideId,
    pub(crate) frame_id: CoordinateFrameId,
    pub(crate) table: MarkTable,
    pub(crate) window: ObservationWindow2D,
}

pub(crate) fn real_fixture(cells_path: &Path, window_path: &Path) -> RealCategoricalFixture {
    let window_text = fs::read_to_string(window_path).expect("real window");
    let mask = TumorMask::from_geojson_str(&window_text).expect("real mask");
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
        StoreId::new("real-cellvit-categorical-store").expect("store ID"),
    )
    .expect("store");
    let probability_provenance = publish_record(
        &mut project,
        &store,
        b"real-cellvit-winning-type-pixel-support",
        MARK_SCHEMA,
        Vec::new(),
        probability_metadata(
            "cellvit_winning_type_pixel_support",
            MeasurementStatus::MorphologyPrediction,
        ),
    );
    let probability_declaration = ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("cellvit_winning_type_pixel_support").expect("probability mark ID"),
        MeasurementStatus::MorphologyPrediction,
        probability_provenance,
    )
    .expect("probability declaration");
    let threshold_provenance = publish_record(
        &mut project,
        &store,
        b"real-cellvit-high-pixel-support-threshold",
        THRESHOLD_SCHEMA,
        vec![probability_provenance],
        threshold_metadata(
            "cellvit_high_type_pixel_support",
            probability_declaration.mark_id().as_str(),
            "greater_than_or_equal",
            0.75,
        ),
    );
    let binary_provenance = publish_record(
        &mut project,
        &store,
        b"real-cellvit-high-pixel-support-binary",
        MARK_SCHEMA,
        vec![probability_provenance, threshold_provenance],
        binary_metadata(
            "cellvit_high_type_pixel_support",
            "CellViT high winner-type pixel support",
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
    let table = MarkTable::new(
        cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::thresholded(
                    ScalarMarkId::new("cellvit_high_type_pixel_support").expect("binary mark ID"),
                    "CellViT high winner-type pixel support",
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
                pattern.categorical_strata["histologic_compartment"].to_vec(),
            )
            .expect("categorical column"),
        ],
        pattern.len(),
        cell_id_text_bytes(&cell_ids),
    )
    .expect("real MarkTable");
    let window =
        ObservationWindow2D::from_geojson_str(&window_text, ObservationWindowLimits::default())
            .expect("observation window")
            .with_coordinate_frame(
                project.coordinate_registry().expect("registry"),
                frame_id.clone(),
            )
            .expect("framed window");

    RealCategoricalFixture {
        semantic_root,
        store,
        project,
        pattern,
        slide_id,
        frame_id,
        table,
        window,
    }
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
