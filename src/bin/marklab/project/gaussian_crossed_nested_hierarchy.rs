use std::{borrow::Cow, path::PathBuf};

use clap::Args;
use marklab_bayes::GaussianCrossedNestedWorkerResult;
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::{
    bayes::{gaussian_crossed_nested_hierarchy, NutsArgs},
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.gaussian-crossed-nested-observations+csv;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-pymc-gaussian-crossed-nested-hierarchy-node-v1";

#[derive(Debug, Args)]
pub(crate) struct ProjectArgs {
    /// Directory of the durable project used to cache this analysis.
    #[arg(long)]
    project: PathBuf,
    /// CSV with patient_id, slide_id, roi_id, batch_id, cohort_id, exposure and outcome columns.
    #[arg(long)]
    input: PathBuf,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior scale for slope.
    #[arg(long)]
    slope_prior_sd: f64,
    /// Prior scale for component.
    #[arg(long)]
    component_prior_sd: f64,
    #[command(flatten)]
    nuts: NutsArgs,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: ProjectArgs) -> Result<(), CliError> {
    const LABEL: &str = "crossed/nested hierarchy input";
    let source = Source::identify(arguments.input.clone(), INPUT_KIND)?;
    let input_path_identity = arguments.input.to_string_lossy().into_owned().into_bytes();
    let prepared = gaussian_crossed_nested_hierarchy::prepare(
        arguments.input,
        arguments.intercept_prior_sd,
        arguments.slope_prior_sd,
        arguments.component_prior_sd,
        arguments.nuts.spec(),
        arguments.timeout_seconds,
    )?;
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "pymc-gaussian-crossed-nested-hierarchy",
        kind: "bayesian_crossed_nested_hierarchy_fit",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            input_path_identity.as_slice(),
            prepared.request_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"pinned-pymc-crossed-nested-hierarchy-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind:
            "application/vnd.marklab.pymc-gaussian-crossed-nested-worker-result+json;version=1",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            gaussian_crossed_nested_hierarchy::execute(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &GaussianCrossedNestedWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &arguments.project,
        &node,
        (
            "marklab.pymc_gaussian_crossed_nested_hierarchy_worker_result",
            1,
        ),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_fit(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "gaussian-crossed-nested-hierarchy",
        &arguments.out,
        &fit,
        run.cache_status,
    )
}
