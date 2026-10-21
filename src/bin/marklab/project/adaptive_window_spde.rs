use std::{borrow::Cow, path::PathBuf};

use marklab_bayes::advanced::{
    validate_adaptive_scientific_result, AdaptiveWindowSpdeResult, AdvancedResult,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::AdaptiveWindowSpdeProjectArgs;
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};
use crate::bayes_advanced::{self, PreparedAdvancedBayes};

const INPUT_KIND: &str = "application/vnd.marklab.source.adaptive-window-spde+json;version=1";
pub(super) const LOCK_KIND: &str = "application/vnd.marklab.python-lock+text;version=1";
pub(super) const WORKER_KIND: &str = "application/vnd.marklab.python-worker+source;version=1";
/// v3: results are stored with the exact-float codec instead of nudged plain-JSON floats.
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-adaptive-window-spde-node-v3";

pub(super) fn run(args: AdaptiveWindowSpdeProjectArgs) -> Result<(), CliError> {
    let AdaptiveWindowSpdeProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    let prepared = bayes_advanced::prepare("adaptive_window_spde", &input_path)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let sources = advanced_bayes_sources(input_path, INPUT_KIND, &prepared)?;
    let contract = NodeContract {
        id: "adaptive-window-spde",
        kind: "adaptive_window_spde",
        version: 1,
        configuration_digest: ContentDigest::from_bytes(&prepared.request_bytes),
        execution_policy: Cow::Borrowed(
            b"bounded-single-process-pinned-scipy;deterministic-delaunay;boundary-refined-exact-window-membership;piecewise-linear-alpha-two-spde;fixed-hyperparameter-one-factor-map",
        ),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind: "application/vnd.marklab.adaptive-window-spde+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        "adaptive SPDE source, environment, or worker",
        sources,
        prepared,
        |prepared| {
            let result = bayes_advanced::execute(prepared).map_err(NodeError::execution)?;
            let AdvancedResult::AdaptiveWindowSpde(result) = result else {
                return Err(NodeError::execution(std::io::Error::other(
                    "advanced Bayesian result identity mismatch",
                )));
            };
            validate_adaptive_scientific_result(&result).map_err(NodeError::execution)?;
            Ok(*result)
        },
        |prepared, result: &AdaptiveWindowSpdeResult| {
            bayes_advanced::validate_adaptive_result(prepared, result)
                .map_err(NodeError::decode)?;
            validate_adaptive_scientific_result(result).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.adaptive_window_spde", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "adaptive-window-spde",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

/// The request's input file plus the worker script and lock it is bound to.
pub(super) fn advanced_bayes_sources(
    input_path: PathBuf,
    input_kind: &'static str,
    prepared: &PreparedAdvancedBayes,
) -> Result<Vec<Source>, CliError> {
    Ok(vec![
        Source::from_bytes(input_path, input_kind, &prepared.input_bytes)?,
        Source::from_bytes(prepared.lock_path(), LOCK_KIND, &prepared.lock_bytes)?,
        Source::from_bytes(
            prepared.worker_path().to_owned(),
            WORKER_KIND,
            &prepared.worker_bytes,
        )?,
    ])
}
