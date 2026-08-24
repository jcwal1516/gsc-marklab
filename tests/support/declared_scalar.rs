use std::collections::BTreeMap;

use marklab::{
    AnalysisConfig, ArtifactDraft, ArtifactId, ArtifactRef, ArtifactSchema, CellId,
    CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, HierarchyId, HierarchyNode, LocalArtifactStore, MarklabProject, Pattern,
    PatternMeta, ReplicationRole, SlideId, SpatialAxis, StoreId, TableColumn, TableColumnType,
    TableFormat, TableManifest, TableScalarType, ThreadSetting,
};
use tempfile::TempDir;

pub(crate) const MARK_SCHEMA: &str = "marklab.scalar_mark_provenance";
pub(crate) const THRESHOLD_SCHEMA: &str = "marklab.scalar_threshold_provenance";

pub(crate) struct Fixture {
    #[allow(dead_code)]
    pub(crate) root: TempDir,
    pub(crate) store: LocalArtifactStore,
    pub(crate) project: MarklabProject,
    pub(crate) pattern: Pattern,
    pub(crate) cell_ids: Vec<CellId>,
    #[allow(dead_code)]
    pub(crate) alternate_cell_ids: Vec<CellId>,
    pub(crate) slide_id: SlideId,
    pub(crate) frame_id: CoordinateFrameId,
    #[allow(dead_code)]
    pub(crate) alternate_frame_id: CoordinateFrameId,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum FrameProfile {
    PhysicalXyMicrometer,
    PhysicalYxMicrometer,
    PhysicalXyzMicrometer,
    PhysicalXyMillimeter,
    ImageXyPixel,
}

pub(crate) fn fixture() -> Fixture {
    fixture_with_frame(Some(FrameProfile::PhysicalXyMicrometer))
}

pub(crate) fn fixture_with_frame(profile: Option<FrameProfile>) -> Fixture {
    let root = TempDir::new().expect("temporary scalar store");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("declared-scalar-store").expect("store ID"),
    )
    .expect("local store");
    let slide_id = SlideId::new("declared-slide").expect("slide ID");
    let foreign_slide_id = SlideId::new("foreign-slide").expect("foreign slide ID");
    let patient =
        HierarchyId::from(marklab::PatientId::new("declared-patient").expect("patient ID"));
    let slide = HierarchyId::from(slide_id.clone());
    let foreign_slide = HierarchyId::from(foreign_slide_id.clone());
    let cell_ids = (0..4)
        .map(|index| CellId::new(format!("cell-{index:04}")).expect("cell ID"))
        .collect::<Vec<_>>();
    let alternate_cell_ids = (0..4)
        .map(|index| CellId::new(format!("other-cell-{index:04}")).expect("alternate cell ID"))
        .collect::<Vec<_>>();
    let foreign_cell = CellId::new("foreign-cell").expect("foreign cell ID");
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient.clone(),
            },
        ),
        HierarchyNode::new(
            foreign_slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
        HierarchyNode::new(
            HierarchyId::from(foreign_cell),
            Some(foreign_slide),
            ReplicationRole::Structural,
        ),
    ];
    nodes.extend(cell_ids.iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(alternate_cell_ids.iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");

    let frame_id = CoordinateFrameId::new("declared-physical-xy").expect("frame ID");
    let alternate_frame_id =
        CoordinateFrameId::new("alternate-physical-xy").expect("alternate frame ID");
    let registry = profile.map(|profile| {
        let (axes, unit, space) = match profile {
            FrameProfile::PhysicalXyMicrometer => (
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Micrometer,
                CoordinateSpace::Physical,
            ),
            FrameProfile::PhysicalYxMicrometer => (
                vec![SpatialAxis::Y, SpatialAxis::X],
                CoordinateUnit::Micrometer,
                CoordinateSpace::Physical,
            ),
            FrameProfile::PhysicalXyzMicrometer => (
                vec![SpatialAxis::X, SpatialAxis::Y, SpatialAxis::Z],
                CoordinateUnit::Micrometer,
                CoordinateSpace::Physical,
            ),
            FrameProfile::PhysicalXyMillimeter => (
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Millimeter,
                CoordinateSpace::Physical,
            ),
            FrameProfile::ImageXyPixel => (
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Pixel,
                CoordinateSpace::Image(marklab::ImageCoordinateConvention::PixelCenterAtInteger),
            ),
        };
        let mut frames =
            vec![CoordinateFrame::new(frame_id.clone(), axes.clone(), unit, space).expect("frame")];
        if matches!(profile, FrameProfile::PhysicalXyMicrometer) {
            frames.push(
                CoordinateFrame::new(alternate_frame_id.clone(), axes, unit, space)
                    .expect("alternate frame"),
            );
        }
        CoordinateRegistry::new(frames, Vec::new(), Vec::new(), Vec::new()).expect("registry")
    });

    let mut project = MarklabProject::new();
    project
        .install_hierarchy(hierarchy)
        .expect("install hierarchy");
    if let Some(registry) = registry {
        project
            .install_coordinate_registry(registry)
            .expect("install registry");
    }

    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![0, 1, 1, 0],
        PatternMeta {
            case_id: "declared-case".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: Some(slide_id.as_str().to_owned()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    pattern.window.area_um2 = 40.0;
    pattern.window.analysis_effective_length_um = 4.0;
    pattern.window.d_nn_mean_um = 1.0;

    Fixture {
        root,
        store,
        project,
        pattern,
        cell_ids,
        alternate_cell_ids,
        slide_id,
        frame_id,
        alternate_frame_id,
    }
}

#[allow(dead_code)]
pub(crate) fn analysis_config(probability: bool) -> AnalysisConfig {
    let mut config = AnalysisConfig::default();
    config.analysis.mark_label = "MMR loss".into();
    config.analysis.use_probabilistic_marks = probability;
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 4;
    config.spectrum.low_k_shells = 1;
    config.spectrum.anisotropy_low_k_shells = 1;
    config.spectrum.fit_low_k_alpha = false;
    config.periodogram.enabled = false;
    config.multiscale_residual.enabled = false;
    config.permutation.b = 39;
    config.permutation.stratified = false;
    config.performance.threads = ThreadSetting::Count(1);
    config.performance.strict_repro = true;
    config
}

pub(crate) fn measurement_status_name(status: marklab::MeasurementStatus) -> &'static str {
    match status {
        marklab::MeasurementStatus::Measured => "measured",
        marklab::MeasurementStatus::ImportedPrediction => "imported_prediction",
        marklab::MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        marklab::MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

pub(crate) fn binary_metadata(
    mark_id: &str,
    label: &str,
    status: marklab::MeasurementStatus,
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

pub(crate) fn probability_metadata(
    mark_id: &str,
    status: marklab::MeasurementStatus,
) -> BTreeMap<String, String> {
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

pub(crate) fn threshold_metadata(
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

pub(crate) fn publish_record(
    fixture: &mut Fixture,
    label: &[u8],
    schema_id: &str,
    schema_version: u32,
    table: Option<TableManifest>,
    dependencies: Vec<ArtifactId>,
    metadata: BTreeMap<String, String>,
) -> ArtifactId {
    let content = ArtifactRef::from_bytes("application/json", label).expect("content ref");
    let draft = ArtifactDraft::new(
        content,
        ArtifactSchema::new(schema_id, schema_version).expect("schema"),
        table,
        dependencies,
        metadata,
    )
    .expect("artifact draft");
    let record = fixture
        .store
        .publish_new_send(&draft, |writer| writer.write_all(label))
        .expect("publish provenance")
        .into_record();
    let id = record.id();
    fixture
        .project
        .register_artifact(record)
        .expect("register provenance");
    id
}

#[allow(dead_code)]
pub(crate) fn one_column_table() -> TableManifest {
    TableManifest::new(
        TableFormat::ParquetFile,
        "test.v1",
        1,
        vec![
            TableColumn::new("id", TableColumnType::Scalar(TableScalarType::Utf8), false)
                .expect("column"),
        ],
        vec!["id".into()],
    )
    .expect("table manifest")
}

pub(crate) fn cell_id_text_bytes(cell_ids: &[CellId]) -> usize {
    cell_ids.iter().map(|id| id.as_str().len()).sum()
}

#[allow(dead_code)]
pub(crate) fn managed_path(root: &std::path::Path, artifact: ArtifactId) -> std::path::PathBuf {
    let id = artifact.to_string();
    root.join("objects/sha256").join(&id[..2]).join(id)
}
