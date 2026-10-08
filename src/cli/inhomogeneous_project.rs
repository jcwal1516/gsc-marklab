use std::{fs, path::Path};

use crate::{
    DurableProject, DurableProjectLimits, LocalArtifactStore, MarklabError, MarklabProject,
    ObservationWindow2D, ObservationWindowLimits, Pattern, PatternLoader, Result, StoreId,
    TumorMask,
};

use super::classical::{read_bounded_utf8, source_artifact};
pub(super) use super::project_output::write_output;

const SOURCE_CELLS_KIND: &str = "application/vnd.marklab.source.point-table;version=1";
const SOURCE_WINDOW_KIND: &str = "application/vnd.marklab.source.observation-window;version=1";
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;

pub(super) struct PrepareRequest<'a> {
    pub project: &'a Path,
    pub cells: &'a Path,
    pub mask: &'a Path,
    pub memory_budget_mib: usize,
    pub store_id: &'static str,
    pub source_change_message: &'static str,
}

pub(super) struct PreparedInhomogeneousProject {
    pub durable: DurableProject,
    pub project: MarklabProject,
    pub store: LocalArtifactStore,
    pub pattern: Pattern,
    pub window: ObservationWindow2D,
    pub memory_bytes: usize,
}

pub(super) fn prepare(request: PrepareRequest<'_>) -> Result<PreparedInhomogeneousProject> {
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
    let window = ObservationWindow2D::from_geojson_str(&window_text, window_limits)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let cells_after = source_artifact(request.cells, SOURCE_CELLS_KIND)?;
    let window_after = source_artifact(request.mask, SOURCE_WINDOW_KIND)?;
    if cells_before != cells_after || window_before != window_after {
        return Err(MarklabError::Validation(
            request.source_change_message.into(),
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
    let project = MarklabProject::with_inline_artifact_limit(memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    Ok(PreparedInhomogeneousProject {
        durable,
        project,
        store,
        pattern,
        window,
        memory_bytes,
    })
}
