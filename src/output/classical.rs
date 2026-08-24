use std::path::{Path, PathBuf};

use marklab_workflow::{ArtifactRef, CacheStatus, ContentDigest};
use serde::Serialize;

use crate::{
    ClassicalCacheStatus, ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialResult,
    ClassicalSpatialResultDocument, ClassicalSpatialStatus, ClassicalWorkflowIdentity, Result,
};

use super::transaction::OutputTransaction;

#[derive(Debug)]
pub(crate) struct ClassicalOutputContext {
    pub(crate) cells: PathBuf,
    pub(crate) mask: PathBuf,
    pub(crate) r_max_um: f64,
    pub(crate) r_steps: usize,
    pub(crate) memory_budget_mib: usize,
    pub(crate) maximum_pair_visits: usize,
    pub(crate) maximum_csr_draws: usize,
    pub(crate) cache_key: ContentDigest,
    pub(crate) cache_status: CacheStatus,
    pub(crate) output_artifact: ArtifactRef,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ClassicalOutputWriter;

impl ClassicalOutputWriter {
    pub(crate) fn write(
        result: &ClassicalSpatialResult,
        out: &Path,
        context: ClassicalOutputContext,
    ) -> Result<()> {
        let document = ClassicalSpatialResultDocument::new(result.clone())?
            .with_workflow_identity(ClassicalWorkflowIdentity {
                cache_key: context.cache_key.to_string(),
                cache_status: match context.cache_status {
                    CacheStatus::Hit => ClassicalCacheStatus::Hit,
                    CacheStatus::Miss => ClassicalCacheStatus::Miss,
                },
                output_artifact_digest: context.output_artifact.digest().to_string(),
                output_artifact_bytes: context.output_artifact.byte_len(),
            })?;
        let result_json = document.to_json_pretty()?;
        let manifest_json =
            serde_json::to_string_pretty(&ClassicalRunManifest::new(result, &context))?;
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
struct ClassicalRunManifest {
    format: &'static str,
    format_version: &'static str,
    command: &'static str,
    program: ProgramManifest,
    inputs: InputManifest,
    configuration: ConfigurationManifest,
    execution: ExecutionManifest,
    scientific_design: ScientificDesignManifest,
    result: ResultManifest,
    artifacts: [&'static str; 3],
}

impl ClassicalRunManifest {
    fn new(result: &ClassicalSpatialResult, context: &ClassicalOutputContext) -> Self {
        Self {
            format: "marklab.classical_spatial.run_manifest",
            format_version: "1",
            command: "classical",
            program: ProgramManifest {
                name: "marklab",
                crate_version: env!("CARGO_PKG_VERSION"),
            },
            inputs: InputManifest {
                cells: context.cells.to_string_lossy().into_owned(),
                mask: context.mask.to_string_lossy().into_owned(),
                pattern_geometry_digest: result.geometry.logical_digest.clone(),
                window_digest: result.window.logical_digest.clone(),
            },
            configuration: ConfigurationManifest {
                logical_digest: result.configuration.logical_digest.clone(),
                r_max_um: context.r_max_um,
                r_steps: context.r_steps,
                simulations: result.null_design.simulations,
                seed: result.null_design.seed,
                alpha: result.null_design.alpha,
                memory_budget_mib: context.memory_budget_mib,
                maximum_pair_visits: context.maximum_pair_visits,
                maximum_csr_draws: context.maximum_csr_draws,
            },
            execution: ExecutionManifest {
                cache_key: context.cache_key.to_string(),
                cache_status: match context.cache_status {
                    CacheStatus::Hit => "hit",
                    CacheStatus::Miss => "miss",
                },
                output_artifact_digest: context.output_artifact.digest().to_string(),
                output_artifact_bytes: context.output_artifact.byte_len(),
                exact_mode: true,
            },
            scientific_design: ScientificDesignManifest {
                estimand: "homogeneous_ripley_k_l",
                edge_correction: "standard_border_reduced_sample",
                null_model: result.null_design.null_model,
                randomization_unit: result.null_design.randomization_unit,
                conditioned_point_count: result.null_design.conditioned_point_count,
            },
            result: ResultManifest {
                case_id: result.case_id.clone(),
                timepoint: result.timepoint.clone(),
                status: result.status,
                radius_count: result.curve.len(),
                point_count: result.geometry.point_count,
            },
            artifacts: ["result.json", "run_manifest.json", "report.md"],
        }
    }
}

#[derive(Serialize)]
struct ProgramManifest {
    name: &'static str,
    crate_version: &'static str,
}

#[derive(Serialize)]
struct InputManifest {
    cells: String,
    mask: String,
    pattern_geometry_digest: String,
    window_digest: String,
}

#[derive(Serialize)]
struct ConfigurationManifest {
    logical_digest: String,
    r_max_um: f64,
    r_steps: usize,
    simulations: usize,
    seed: u64,
    alpha: f64,
    memory_budget_mib: usize,
    maximum_pair_visits: usize,
    maximum_csr_draws: usize,
}

#[derive(Serialize)]
struct ExecutionManifest {
    cache_key: String,
    cache_status: &'static str,
    output_artifact_digest: String,
    output_artifact_bytes: u64,
    exact_mode: bool,
}

#[derive(Serialize)]
struct ScientificDesignManifest {
    estimand: &'static str,
    edge_correction: &'static str,
    null_model: ClassicalNullModel,
    randomization_unit: ClassicalRandomizationUnit,
    conditioned_point_count: usize,
}

#[derive(Serialize)]
struct ResultManifest {
    case_id: String,
    timepoint: String,
    status: ClassicalSpatialStatus,
    radius_count: usize,
    point_count: usize,
}

fn render_report(result: &ClassicalSpatialResult) -> String {
    let inference = result.inference.as_ref().map_or_else(
        || "Global inference: unavailable under the typed result status.".to_owned(),
        |summary| {
            format!(
                "Global inference: ERL p = {:.6}; {} radius value(s) were jointly eligible.",
                summary.p_global, summary.eligible_radius_count
            )
        },
    );
    format!(
        "# Marklab classical spatial-pathology report\n\n\
## Workflow\n\n\
Homogeneous Ripley K and L were estimated for the retained unmarked simple point pattern in the explicit two-dimensional observation window. The estimator uses the standard border (reduced-sample) correction.\n\n\
- Case: `{}`\n\
- Timepoint: `{}`\n\
- Workflow status: `{:?}`\n\
- Retained points: {}\n\
- Window area: {:.6} µm²\n\
- Window perimeter: {:.6} µm\n\
- Radius values: {}\n\
- Observed ordered-neighbor visits: {}\n\n\
## Null and inference\n\n\
The null is conditional homogeneous CSR: the validated window and observed point count are fixed, and the randomization unit is the whole location pattern. Cells are not treated as independent biological replicates. {}\n\n\
## Claim boundary\n\n\
Departure from conditional CSR does not establish a biological mechanism. This workflow does not provide patient-level inference, treatment-effect inference, causal evidence, or clinical validation. Marks are outside this unmarked location-process estimand.\n",
        result.case_id,
        result.timepoint,
        result.status,
        result.geometry.point_count,
        result.window.area_um2,
        result.window.perimeter_um,
        result.curve.len(),
        result.geometry.observed_pair_visits,
        inference,
    )
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, text).map_err(|source| crate::MarklabError::io(path, source))
}
