use serde::Serialize;

use crate::config::OutputSection;

use super::{AnalysisResult, ResultDocument, StatusFlag, RESULT_FORMAT_VERSION};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    program: &'static str,
    crate_version: &'static str,
    format_version: &'static str,
    analysis_kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    inputs: Option<RunManifestInputs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution: Option<RunManifestExecution>,
    result: RunManifestResult,
    output: RunManifestOutput,
    timings_stage_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunManifestInputs {
    pub(crate) cells: String,
    pub(crate) mask: String,
    pub(crate) config: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunManifestExecution {
    pub(crate) thread_count: usize,
    pub(crate) requested_threads: Option<usize>,
    pub(crate) permutations: usize,
    pub(crate) permutation_seed: u64,
    pub(crate) strict_repro: bool,
    pub(crate) save_intermediates: bool,
    pub(crate) log_level: Option<String>,
    pub(crate) heap_profile: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct RunManifestContext {
    pub(crate) command: String,
    pub(crate) inputs: RunManifestInputs,
    pub(crate) execution: RunManifestExecution,
}

#[derive(Clone, Debug, Serialize)]
struct RunManifestResult {
    case_id: String,
    timepoint: String,
    protein: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mark_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_flags: Option<Vec<StatusFlag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n_cells: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n_marked: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p_hat: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct RunManifestOutput {
    write_parquet_curves: bool,
    write_geojson_territories: bool,
    write_figures: bool,
    write_run_manifest: bool,
}

impl RunManifest {
    pub(crate) fn from_document(
        document: &ResultDocument,
        options: &OutputSection,
        context: Option<RunManifestContext>,
    ) -> Self {
        let (analysis_kind, result, timings_stage_count) = match &document.analysis {
            AnalysisResult::MarkedPattern(result) => (
                "marked_pattern",
                RunManifestResult {
                    case_id: result.case_id.clone(),
                    timepoint: result.timepoint.clone(),
                    protein: result.protein.clone(),
                    status: result.status.clone(),
                    mark_label: Some(result.mark_label.clone()),
                    status_flags: Some(result.status_flags.clone()),
                    n_cells: Some(result.n_cells),
                    n_marked: Some(result.n_marked),
                    p_hat: Some(result.p_hat),
                },
                result.timings.len(),
            ),
            AnalysisResult::Multimodal(result) => (
                "multimodal",
                RunManifestResult {
                    case_id: result.case_id.clone(),
                    timepoint: result.timepoint.clone(),
                    protein: result.protein.clone(),
                    status: result.status.clone(),
                    mark_label: None,
                    status_flags: None,
                    n_cells: None,
                    n_marked: None,
                    p_hat: None,
                },
                result.timings.len(),
            ),
        };
        let (command, inputs, execution) = match context {
            Some(context) => (
                Some(context.command),
                Some(context.inputs),
                Some(context.execution),
            ),
            None => (None, None, None),
        };

        Self {
            command,
            program: "marklab",
            crate_version: env!("CARGO_PKG_VERSION"),
            format_version: RESULT_FORMAT_VERSION,
            analysis_kind,
            inputs,
            execution,
            result,
            output: RunManifestOutput {
                write_parquet_curves: options.write_parquet_curves,
                write_geojson_territories: options.write_geojson_territories,
                write_figures: options.write_figures,
                write_run_manifest: options.write_run_manifest,
            },
            timings_stage_count,
        }
    }
}
