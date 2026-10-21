use std::{borrow::Cow, path::PathBuf};

use marklab_bayes::NegativeBinomialHierarchyResult;
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::NegativeBinomialHierarchyProjectArgs;
use super::{
    bayes::negative_binomial_hierarchy::{self, Prepared},
    durable::{self, Codec, NodeContract, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const IMPLEMENTATION_IDENTITY: &str = "marklab-project-negative-binomial-hierarchy-v1";

pub(super) fn run(args: NegativeBinomialHierarchyProjectArgs) -> Result<(), CliError> {
    let NegativeBinomialHierarchyProjectArgs {
        project: project_path,
        input,
        out: output,
    } = args;
    // Validate the full design and budgets before creating durable state.
    let prepared = negative_binomial_hierarchy::prepare(&input)?;
    let contract = NodeContract {
        id: "negative-binomial-hierarchy",
        kind: "bayesian_negative_binomial_patient_slide_fit",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            prepared.request_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"bounded-pinned-numpyro-negative-binomial-hierarchy-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind: "application/vnd.marklab.negative-binomial-hierarchy+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::with_input_check(
        contract,
        prepared.artifacts.clone(),
        (input, prepared),
        |(input, prepared)| {
            let current = negative_binomial_hierarchy::prepare(input).map_err(NodeError::input)?;
            if current.artifacts != prepared.artifacts
                || current.request_bytes != prepared.request_bytes
            {
                return Err(NodeError::input(CliError::Input(
                    "negative-binomial source, worker, lock or request changed after preparation"
                        .into(),
                )));
            }
            Ok(())
        },
        |(_, prepared)| {
            negative_binomial_hierarchy::execute(prepared).map_err(NodeError::execution)
        },
        |(_, prepared): &(PathBuf, Prepared), result: &NegativeBinomialHierarchyResult| {
            result
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.negative_binomial_hierarchy", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "negative-binomial-hierarchy",
        &output,
        &run.output,
        run.cache_status,
    )
}
