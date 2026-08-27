use std::path::{Path, PathBuf};

use marklab_workflow::{ArtifactRef, CacheStatus, ContentDigest};
use serde::Serialize;

use crate::{
    NearestSpaceCacheStatus, NearestSpaceResult, NearestSpaceResultDocument,
    NearestSpaceWorkflowIdentity, Result,
};

use super::transaction::OutputTransaction;

#[derive(Debug)]
pub(crate) struct NearestSpaceOutputContext {
    pub(crate) command: &'static str,
    pub(crate) cells: PathBuf,
    pub(crate) mask: PathBuf,
    pub(crate) memory_budget_mib: usize,
    pub(crate) cache_key: ContentDigest,
    pub(crate) cache_status: CacheStatus,
    pub(crate) output_artifact: ArtifactRef,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NearestSpaceOutputWriter;

impl NearestSpaceOutputWriter {
    pub(crate) fn write(
        result: &NearestSpaceResult,
        out: &Path,
        context: NearestSpaceOutputContext,
    ) -> Result<()> {
        let document = NearestSpaceResultDocument::new(result.clone())?.with_workflow_identity(
            NearestSpaceWorkflowIdentity {
                cache_key: context.cache_key.to_string(),
                cache_status: match context.cache_status {
                    CacheStatus::Hit => NearestSpaceCacheStatus::Hit,
                    CacheStatus::Miss => NearestSpaceCacheStatus::Miss,
                },
                output_artifact_digest: context.output_artifact.digest().to_string(),
                output_artifact_bytes: context.output_artifact.byte_len(),
            },
        )?;
        let result_json = document.to_json_pretty()?;
        let manifest_json = serde_json::to_string_pretty(&RunManifest::new(result, &context))?;
        let report = render_report(result);

        let transaction = OutputTransaction::new(out)?;
        let staging = transaction.staging_path();
        write_text(&staging.join("result.json"), &result_json)?;
        write_text(&staging.join("run_manifest.json"), &manifest_json)?;
        write_text(&staging.join("report.md"), &report)?;
        transaction.commit()
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RunManifest {
    format: &'static str,
    format_version: &'static str,
    command: &'static str,
    crate_version: &'static str,
    cells: String,
    mask: String,
    window_digest: String,
    geometry_digest: String,
    probe_digest: String,
    configuration_digest: String,
    memory_budget_mib: usize,
    cache_key: String,
    cache_status: &'static str,
    output_artifact_digest: String,
    output_artifact_bytes: u64,
    estimands: [&'static str; 3],
    edge_correction: &'static str,
    probe_method: &'static str,
    null_model: &'static str,
    randomization_unit: &'static str,
    point_count: usize,
    retained_probe_count: usize,
    artifacts: [&'static str; 3],
}

impl RunManifest {
    fn new(result: &NearestSpaceResult, context: &NearestSpaceOutputContext) -> Self {
        Self {
            format: "marklab.nearest_space.run_manifest",
            format_version: "1",
            command: context.command,
            crate_version: env!("CARGO_PKG_VERSION"),
            cells: context.cells.to_string_lossy().into_owned(),
            mask: context.mask.to_string_lossy().into_owned(),
            window_digest: result.window.logical_digest.clone(),
            geometry_digest: result.geometry.logical_digest.clone(),
            probe_digest: result.probes.logical_digest.clone(),
            configuration_digest: result.configuration.logical_digest.clone(),
            memory_budget_mib: context.memory_budget_mib,
            cache_key: context.cache_key.to_string(),
            cache_status: match context.cache_status {
                CacheStatus::Hit => "hit",
                CacheStatus::Miss => "miss",
            },
            output_artifact_digest: context.output_artifact.digest().to_string(),
            output_artifact_bytes: context.output_artifact.byte_len(),
            estimands: ["empty_space_f", "nearest_neighbor_g", "j_ratio"],
            edge_correction: "standard_border_reduced_sample",
            probe_method: "fixed_cell_centred_rectangular_grid",
            null_model: "homogeneous_csr_conditional_on_count",
            randomization_unit: "whole_location_pattern",
            point_count: result.geometry.point_count,
            retained_probe_count: result.probes.retained_probe_count,
            artifacts: ["result.json", "run_manifest.json", "report.md"],
        }
    }
}

fn render_report(result: &NearestSpaceResult) -> String {
    let component = |name: &str, value: Option<&crate::NearestSpaceComponentInference>| {
        value.map_or_else(
            || format!("{name}: unavailable under the typed support rules."),
            |value| {
                format!(
                    "{name}: ERL p = {:.6} over {} jointly eligible radius value(s).",
                    value.p_global, value.eligible_radius_count
                )
            },
        )
    };
    format!(
        "# Marklab nearest/empty-space report\n\n\
## Workflow\n\n\
F, G, and J were estimated for one unmarked point pattern in an exact two-dimensional observation window. F uses fixed cell-centred probes, G uses each event's nearest distinct event, and both use the standard reduced-sample border correction. J is omitted wherever `1-F` does not exceed the declared denominator floor.\n\n\
- Case: `{}`\n\
- Timepoint: `{}`\n\
- Workflow status: `{:?}`\n\
- Retained events: {}\n\
- Retained deterministic probes: {}\n\
- Probe grid: {} × {}\n\
- Maximum probe location displacement: {:.6} µm\n\
- Exact nearest queries: {}\n\n\
## Null and inference\n\n\
The null is homogeneous CSR conditional on the observed event count. The randomization unit is the whole location pattern; the same fixed probes and window are reused for every null pattern. {} {} {}\n\n\
## Claim boundary\n\n\
F, G, and J describe empty space and event spacing relative to conditional CSR. They do not establish a biological mechanism and do not provide patient-level inference, treatment effects, causal evidence, or clinical validation. Deterministic grid probes are not represented as random Monte Carlo samples.\n",
        result.case_id,
        result.timepoint,
        result.status,
        result.geometry.point_count,
        result.probes.retained_probe_count,
        result.probes.requested_grid[0],
        result.probes.requested_grid[1],
        result.probes.maximum_location_error_um,
        result.geometry.nearest_query_count,
        component("F", result.inference.f.as_ref()),
        component("G", result.inference.g.as_ref()),
        component("J", result.inference.j.as_ref()),
    )
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, text).map_err(|source| crate::MarklabError::io(path, source))
}
