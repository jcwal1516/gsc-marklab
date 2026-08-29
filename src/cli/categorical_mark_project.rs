use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use crate::{
    ArtifactDraft, ArtifactId, ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CacheStatus,
    CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, DurableProject, DurableProjectLimits, HierarchyId, HierarchyNode,
    HistologicCompartmentMarkDeclaration, LocalArtifactStore, MarkTable, MarklabError,
    MarklabProject, MeasurementStatus, MissingnessPolicy, ObservationWindow2D,
    ObservationWindowLimits, PatientId, Pattern, PatternLoader, ReplicationRole, Result,
    ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, SlideId, SpatialAxis,
    StoreId, TumorMask,
};

use super::classical::{read_bounded_utf8, source_artifact};

const SOURCE_CELLS_KIND: &str = "application/vnd.marklab.source.categorical-cell-table;version=1";
const SOURCE_WINDOW_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";
const MARK_SCHEMA: &str = "marklab.scalar_mark_provenance";
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;
const FRAME_ID: &str = "categorical-pair-physical-xy-um";

pub(super) struct PrepareRequest<'a> {
    pub project: &'a Path,
    pub cells: &'a Path,
    pub mask: &'a Path,
    pub memory_budget_mib: usize,
    pub store_id: &'static str,
}

pub(super) struct PreparedCategoricalMarkProject {
    pub durable: DurableProject,
    pub project: MarklabProject,
    pub store: LocalArtifactStore,
    pub pattern: Pattern,
    pub table: MarkTable,
    pub window: ObservationWindow2D,
    pub slide_id: SlideId,
    pub frame_id: CoordinateFrameId,
    pub memory_bytes: usize,
}

pub(super) fn prepare(request: PrepareRequest<'_>) -> Result<PreparedCategoricalMarkProject> {
    let cells_before = source_artifact(request.cells, SOURCE_CELLS_KIND)?;
    let window_before = source_artifact(request.mask, SOURCE_WINDOW_KIND)?;
    let memory_bytes = request
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| MarklabError::Validation("--memory-budget-mib is too large".into()))?;
    if memory_bytes == 0 {
        return Err(MarklabError::Validation(
            "--memory-budget-mib must be positive".into(),
        ));
    }
    let window_limits = ObservationWindowLimits::default();
    let window_text = read_bounded_utf8(request.mask, window_limits.maximum_input_bytes)?;
    let mask = TumorMask::from_geojson_str(&window_text)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let pattern = PatternLoader::new(&mask).load(request.cells)?;
    let cell_ids = pattern
        .typed_cell_ids()?
        .ok_or_else(|| {
            MarklabError::Validation(
                "categorical mark workflow requires stable cell_id rows".into(),
            )
        })?
        .into_vec();
    let levels = pattern
        .categorical_stratum_levels
        .get("histologic_compartment")
        .ok_or_else(|| {
            MarklabError::Validation(
                "categorical mark workflow requires the histologic_compartment codebook".into(),
            )
        })?
        .to_vec();
    let codes = pattern
        .categorical_strata
        .get("histologic_compartment")
        .ok_or_else(|| {
            MarklabError::Validation(
                "categorical mark workflow requires histologic_compartment codes".into(),
            )
        })?
        .to_vec();
    let slide_id = SlideId::new(pattern.meta.slide_id.as_deref().ok_or_else(|| {
        MarklabError::Validation("categorical mark workflow slide_id is absent".into())
    })?)
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let patient_id = PatientId::new(&pattern.meta.case_id)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let frame_id = CoordinateFrameId::new(FRAME_ID)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let hierarchy = hierarchy(&cell_ids, patient_id, slide_id.clone())?;
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .map_err(|error| MarklabError::Validation(error.to_string()))?],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let window = ObservationWindow2D::from_geojson_str(&window_text, window_limits)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;

    let cells_after = source_artifact(request.cells, SOURCE_CELLS_KIND)?;
    let window_after = source_artifact(request.mask, SOURCE_WINDOW_KIND)?;
    if cells_before != cells_after || window_before != window_after {
        return Err(MarklabError::Validation(
            "categorical mark source changed while its durable input was prepared".into(),
        ));
    }

    let durable_limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let durable = DurableProject::open_or_create(request.project, durable_limits)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let store_path = request.project.join(request.store_id);
    fs::create_dir_all(&store_path).map_err(|source| MarklabError::io(&store_path, source))?;
    let store = LocalArtifactStore::open(
        &store_path,
        StoreId::new(request.store_id)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let mut project = MarklabProject::with_inline_artifact_limit(memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    project
        .install_hierarchy(hierarchy)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    project
        .install_coordinate_registry(registry)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let source_identity = BTreeMap::from([
        ("cells_sha256".into(), cells_before.digest().to_string()),
        ("window_sha256".into(), window_before.digest().to_string()),
    ]);
    let binary_provenance = publish_record(
        &mut project,
        &store,
        "categorical-pair-binary-input",
        BTreeMap::from([
            ("mark_id".into(), "binary_mark".into()),
            ("mark_label".into(), "Imported binary mark".into()),
            ("measurement_status".into(), "imported_prediction".into()),
            ("origin".into(), "independent".into()),
            ("unit".into(), "unitless".into()),
            ("value_kind".into(), "binary".into()),
        ]),
        &source_identity,
    )?;
    let level_refs = levels.iter().map(String::as_str).collect::<Vec<_>>();
    let categorical_provenance = publish_record(
        &mut project,
        &store,
        "categorical-pair-histologic-compartment",
        BTreeMap::from([
            (
                "levels_digest".into(),
                crate::ContentDigest::from_framed(level_refs.iter().map(|level| level.as_bytes()))
                    .to_string(),
            ),
            ("levels_count".into(), levels.len().to_string()),
            ("mark_id".into(), "histologic_compartment".into()),
            ("mark_label".into(), "Histologic compartment".into()),
            ("measurement_status".into(), "morphology_prediction".into()),
            ("modality".into(), "histology".into()),
            ("unit".into(), "categorical".into()),
            ("value_kind".into(), "categorical".into()),
        ]),
        &source_identity,
    )?;
    let table = MarkTable::new(
        cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                BinaryMarkDeclaration::independent(
                    ScalarMarkId::new("binary_mark")
                        .map_err(|error| MarklabError::Validation(error.to_string()))?,
                    "Imported binary mark",
                    MeasurementStatus::ImportedPrediction,
                    binary_provenance,
                )
                .map_err(|error| MarklabError::Validation(error.to_string()))?,
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                pattern.mark.clone(),
            )
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
            ScalarMarkColumn::histologic_compartment(
                HistologicCompartmentMarkDeclaration::new(
                    levels,
                    MeasurementStatus::MorphologyPrediction,
                    categorical_provenance,
                )
                .map_err(|error| MarklabError::Validation(error.to_string()))?,
                ScalarMarkModality::Histology,
                ScalarMarkUnit::Categorical,
                MissingnessPolicy::NotPermitted,
                codes,
            )
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        ],
        pattern.len(),
        cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let window = window
        .with_coordinate_frame(
            project
                .coordinate_registry()
                .ok_or_else(|| MarklabError::Validation("coordinate registry is absent".into()))?,
            frame_id.clone(),
        )
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    Ok(PreparedCategoricalMarkProject {
        durable,
        project,
        store,
        pattern,
        table,
        window,
        slide_id,
        frame_id,
        memory_bytes,
    })
}

pub(super) fn write_output(
    path: &Path,
    encoded: &[u8],
    cache_status: CacheStatus,
    command: &str,
) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| MarklabError::io(path, source))?;
    output
        .write_all(encoded)
        .map_err(|source| MarklabError::io(path, source))?;
    output
        .sync_all()
        .map_err(|source| MarklabError::io(path, source))?;
    let cache = match cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project {command} cache_status={cache}");
    Ok(())
}

fn hierarchy(
    cell_ids: &[crate::CellId],
    patient_id: PatientId,
    slide_id: SlideId,
) -> Result<CohortHierarchy> {
    let patient = HierarchyId::from(patient_id);
    let slide = HierarchyId::from(slide_id);
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
    CohortHierarchy::new(nodes, Vec::new())
        .map_err(|error| MarklabError::Validation(error.to_string()))
}

fn publish_record(
    project: &mut MarklabProject,
    store: &LocalArtifactStore,
    role: &str,
    metadata: BTreeMap<String, String>,
    source_identity: &BTreeMap<String, String>,
) -> Result<ArtifactId> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "role": role,
        "source_identity": source_identity,
    }))
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let draft = ArtifactDraft::new(
        ArtifactRef::from_bytes("application/json", &bytes)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        ArtifactSchema::new(MARK_SCHEMA, 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        None,
        Vec::new(),
        metadata,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let record = store
        .publish_new_send(&draft, |writer| writer.write_all(&bytes))
        .map_err(|error| MarklabError::Compute(error.to_string()))?
        .into_record();
    let id = record.id();
    project
        .register_artifact(record)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    Ok(id)
}
