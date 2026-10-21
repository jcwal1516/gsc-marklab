use std::borrow::Cow;

use marklab_bayes::advanced::{AdvancedResult, NonstationaryAdaptiveWindowSpdeResult};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::NonstationaryAdaptiveWindowSpdeProjectArgs;
use super::{
    adaptive_window_spde::advanced_bayes_sources,
    durable::{self, Codec, NodeContract, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};
use crate::bayes_advanced;

const INPUT_KIND: &str =
    "application/vnd.marklab.source.nonstationary-adaptive-window-spde+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-nonstationary-adaptive-window-spde-node-v1";

pub(super) fn run(args: NonstationaryAdaptiveWindowSpdeProjectArgs) -> Result<(), CliError> {
    let NonstationaryAdaptiveWindowSpdeProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    let prepared = bayes_advanced::prepare("nonstationary_adaptive_window_spde", &input_path)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let sources = advanced_bayes_sources(input_path, INPUT_KIND, &prepared)?;
    let contract = NodeContract {
        id: "nonstationary-adaptive-window-spde",
        kind: "nonstationary_adaptive_spatial_field_fit",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            prepared.request_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"bounded-pinned-scipy-nonstationary-adaptive-spde-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind: "application/vnd.marklab.nonstationary-adaptive-window-spde+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        "nonstationary adaptive SPDE inputs",
        sources,
        prepared,
        |prepared| {
            let result = bayes_advanced::execute(prepared).map_err(NodeError::execution)?;
            let AdvancedResult::NonstationaryAdaptiveWindowSpde(result) = result else {
                return Err(NodeError::execution(std::io::Error::other(
                    "advanced Bayesian result identity mismatch",
                )));
            };
            Ok(*result)
        },
        |prepared, output: &NonstationaryAdaptiveWindowSpdeResult| {
            bayes_advanced::validate_nonstationary_result(prepared, output)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.nonstationary_adaptive_window_spde", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "nonstationary-adaptive-window-spde",
        &output_path,
        &serde_json::to_value(&run.output)?,
        run.cache_status,
    )
}
