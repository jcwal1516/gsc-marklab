use std::{fs::File, io::Read};

use crate::{
    geom::{mask::TumorMask, window::ObservationWindowLimits},
    output::{ClassicalOutputContext, ClassicalOutputWriter},
    ClassicalSpatialAnalysisNode, ClassicalSpatialConfig, ClassicalSpatialLimits, LocalScheduler,
    MarklabError, MarklabProject, NodeId, ObservationWindow2D, PatternLoader, Result,
    SchedulerLimits, WorkflowGraph,
};

use super::ClassicalRequest;

const MAXIMUM_RADII: usize = 4_096;

pub(super) fn run(request: ClassicalRequest) -> Result<()> {
    validate_request(&request)?;
    let memory_bytes = request
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| MarklabError::Validation("--memory-budget-mib is too large".into()))?;
    let window_limits = ObservationWindowLimits::default();
    let window_text = read_bounded_utf8(&request.mask, window_limits.maximum_input_bytes)?;
    let window = ObservationWindow2D::from_geojson_str(&window_text, window_limits)
        .map_err(|error| MarklabError::Geometry(error.to_string()))?;
    let mask = TumorMask::from_observation_window(window.clone());
    let pattern = PatternLoader::new(&mask).load_classical(&request.cells)?;

    let radii = (1..=request.r_steps)
        .map(|index| request.r_max_um * (index as f64 / request.r_steps as f64))
        .collect::<Vec<_>>();
    let maximum_points = (memory_bytes / (2 * std::mem::size_of::<f64>())).max(1);
    let limits = ClassicalSpatialLimits::new(
        maximum_points,
        MAXIMUM_RADII,
        request.maximum_pair_visits,
        request.maximum_csr_draws,
        memory_bytes,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let config = ClassicalSpatialConfig::new(
        radii,
        request.simulations,
        request.seed,
        request.alpha,
        limits,
    )
    .map_err(|error| MarklabError::Validation(error.to_string()))?;

    let mut project = MarklabProject::with_inline_artifact_limit(memory_bytes)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let node = ClassicalSpatialAnalysisNode::new(
        &mut project,
        NodeId::new("classical-spatial-analysis")
            .map_err(|error| MarklabError::Validation(error.to_string()))?,
        &pattern,
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
    let run = scheduler
        .run_single(&mut project, &graph, &node)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;

    ClassicalOutputWriter::write(
        &run.output,
        &request.out,
        ClassicalOutputContext {
            cells: request.cells,
            mask: request.mask,
            r_max_um: request.r_max_um,
            r_steps: request.r_steps,
            memory_budget_mib: request.memory_budget_mib,
            maximum_pair_visits: request.maximum_pair_visits,
            maximum_csr_draws: request.maximum_csr_draws,
            cache_key: run.cache_key,
            cache_status: run.cache_status,
            output_artifact: run.artifact,
        },
    )
}

fn validate_request(request: &ClassicalRequest) -> Result<()> {
    if !request.r_max_um.is_finite() || request.r_max_um <= 0.0 {
        return Err(MarklabError::Validation(
            "--r-max-um must be finite and positive".into(),
        ));
    }
    if request.r_steps == 0 || request.r_steps > MAXIMUM_RADII {
        return Err(MarklabError::Validation(format!(
            "--r-steps must be in 1..={MAXIMUM_RADII}"
        )));
    }
    if request.simulations == 0
        || !request.alpha.is_finite()
        || request.alpha <= 0.0
        || request.alpha >= 1.0
        || (request.simulations.saturating_add(1) as f64) * request.alpha < 1.0
    {
        return Err(MarklabError::Validation(
            "--simulations must be positive, --alpha must be in (0, 1), and (simulations + 1) * alpha must be at least one".into(),
        ));
    }
    if request.memory_budget_mib == 0
        || request.maximum_pair_visits == 0
        || request.maximum_csr_draws == 0
    {
        return Err(MarklabError::Validation(
            "classical memory, pair-visit, and CSR-draw limits must be positive".into(),
        ));
    }
    Ok(())
}

fn read_bounded_utf8(path: &std::path::Path, maximum_bytes: usize) -> Result<String> {
    let file = File::open(path).map_err(|source| MarklabError::io(path, source))?;
    let limit = u64::try_from(maximum_bytes)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| MarklabError::Validation("window byte limit is too large".into()))?;
    let mut text = String::new();
    file.take(limit)
        .read_to_string(&mut text)
        .map_err(|source| MarklabError::io(path, source))?;
    if text.len() > maximum_bytes {
        return Err(MarklabError::Geometry(format!(
            "window GeoJSON has more than {maximum_bytes} bytes"
        )));
    }
    Ok(text)
}
