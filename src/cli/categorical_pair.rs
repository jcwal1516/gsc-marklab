use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use crate::{
    categorical_pair_workflow, execute_algorithm_with_store, ArtifactDraft, ArtifactId,
    ArtifactRef, ArtifactSchema, BinaryMarkDeclaration, CacheStatus, CategoricalPairAnalysisNode,
    CategoricalPairConfig, CategoricalPairLimits, CohortHierarchy, CoordinateFrame,
    CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    DeclaredScalarPatternInput, DurableProject, DurableProjectLimits, HierarchyId, HierarchyNode,
    HistologicCompartmentMarkDeclaration, LocalArtifactStore, LocalScheduler, MarkTable,
    MarklabError, MarklabProject, MeasurementStatus, MissingnessPolicy, NodeId,
    ObservationWindow2D, ObservationWindowLimits, PatientId, PatternLoader, ReplicationRole,
    Result, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, SchedulerLimits,
    SlideId, SpatialAxis, StoreId, TumorMask, WorkflowGraph,
};

use super::classical::{native_runtime_provenance, source_artifact};

const SOURCE_CELLS_KIND: &str = "application/vnd.marklab.source.categorical-cell-table;version=1";
const SOURCE_WINDOW_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";
const MARK_SCHEMA: &str = "marklab.scalar_mark_provenance";
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;

pub(super) struct Request {
    pub project: PathBuf,
    pub cells: PathBuf,
    pub mask: PathBuf,
    pub out: PathBuf,
    pub source_level: String,
    pub target_level: String,
    pub radii_um: Vec<f64>,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub memory_budget_mib: usize,
    pub maximum_pair_visits: usize,
    pub maximum_null_pair_evaluations: usize,
}

pub(super) fn run_project(request: Request) -> Result<()> {
    let cells_before = source_artifact(&request.cells, SOURCE_CELLS_KIND)?;
    let window_before = source_artifact(&request.mask, SOURCE_WINDOW_KIND)?;
    let memory_bytes = request
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| MarklabError::Validation("--memory-budget-mib is too large".into()))?;
    if memory_bytes == 0 {
        return Err(MarklabError::Validation(
            "--memory-budget-mib must be positive".into(),
        ));
    }
    let window_text = read_bounded_window(&request.mask)?;
    let mask = TumorMask::from_geojson_str(&window_text)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let pattern = PatternLoader::new(&mask).load(&request.cells)?;
    let cell_ids = pattern
        .typed_cell_ids()?
        .ok_or_else(|| {
            MarklabError::Validation("categorical pair requires stable cell_id rows".into())
        })?
        .into_vec();
    let levels = pattern
        .categorical_stratum_levels
        .get("histologic_compartment")
        .ok_or_else(|| {
            MarklabError::Validation(
                "categorical pair requires the histologic_compartment codebook".into(),
            )
        })?
        .to_vec();
    let codes = pattern
        .categorical_strata
        .get("histologic_compartment")
        .ok_or_else(|| {
            MarklabError::Validation(
                "categorical pair requires histologic_compartment codes".into(),
            )
        })?
        .to_vec();
    let slide_id =
        SlideId::new(pattern.meta.slide_id.as_deref().ok_or_else(|| {
            MarklabError::Validation("categorical pair slide_id is absent".into())
        })?)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let patient_id = PatientId::new(&pattern.meta.case_id)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let frame_id = CoordinateFrameId::new("categorical-pair-physical-xy-um")
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
    let window =
        ObservationWindow2D::from_geojson_str(&window_text, ObservationWindowLimits::default())
            .map_err(|error| MarklabError::Geometry(error.to_string()))?;

    let cells_after = source_artifact(&request.cells, SOURCE_CELLS_KIND)?;
    let window_after = source_artifact(&request.mask, SOURCE_WINDOW_KIND)?;
    if cells_before != cells_after || window_before != window_after {
        return Err(MarklabError::Validation(
            "categorical pair source changed while its durable input was prepared".into(),
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
    let mut durable = DurableProject::open_or_create(&request.project, durable_limits)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let store_path = request.project.join("categorical-pair-store");
    fs::create_dir_all(&store_path).map_err(|source| MarklabError::io(&store_path, source))?;
    let store = LocalArtifactStore::open(
        &store_path,
        StoreId::new("categorical-pair-store")
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
    let input = DeclaredScalarPatternInput::from_mark_table(
        &project,
        &pattern,
        &table,
        slide_id,
        frame_id.clone(),
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let window = window
        .with_coordinate_frame(
            project
                .coordinate_registry()
                .ok_or_else(|| MarklabError::Validation("coordinate registry is absent".into()))?,
            frame_id,
        )
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let limits = CategoricalPairLimits::new(
        pattern.len(),
        request.radii_um.len(),
        request.maximum_pair_visits,
        request.maximum_null_pair_evaluations,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = CategoricalPairConfig::new(
        request.radii_um,
        request.source_level,
        request.target_level,
        request.permutations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = CategoricalPairAnalysisNode::new(
        &mut project,
        NodeId::new("categorical-pair")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &input,
        &window,
        &config,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: memory_bytes,
    })
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let run = execute_algorithm_with_store(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.categorical_pair", 1)
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        native_runtime_provenance()?,
        &store,
    )
    .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let encoded = categorical_pair_workflow::encode_result(&run.output)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&request.out)
        .map_err(|source| MarklabError::io(&request.out, source))?;
    output
        .write_all(&encoded)
        .map_err(|source| MarklabError::io(&request.out, source))?;
    output
        .sync_all()
        .map_err(|source| MarklabError::io(&request.out, source))?;
    let cache = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project categorical-pair cache_status={cache}");
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

fn read_bounded_window(path: &PathBuf) -> Result<String> {
    let metadata = fs::metadata(path).map_err(|source| MarklabError::io(path, source))?;
    let maximum = ObservationWindowLimits::default().maximum_input_bytes as u64;
    if metadata.len() > maximum {
        return Err(MarklabError::Validation(format!(
            "observation window exceeds {maximum} bytes"
        )));
    }
    fs::read_to_string(path).map_err(|source| MarklabError::io(path, source))
}
