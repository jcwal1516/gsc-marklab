use std::borrow::Cow;

use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::super::bayes::correlated_replicated_arbitrary_window_multitype_lgcp::{
    self, Args, Output,
};
use super::{
    cli_schema::InProject,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-numpyro-correlated-replicated-multitype-lgcp-node-v2";

pub(super) type ProjectArgs = InProject<Args>;

pub(super) fn run(arguments: ProjectArgs) -> Result<(), CliError> {
    const LABEL: &str = "correlated multitype source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = arguments;
    let source = Source::identify(arguments.input.clone(), INPUT_KIND)?;
    let output_path = std::mem::take(&mut arguments.out);
    let prepared = correlated_replicated_arbitrary_window_multitype_lgcp::prepare(arguments)?;
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "correlated-replicated-multitype-lgcp",
        kind: "bayesian_correlated_multitype_exact_window_point_process",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            prepared.request_bytes(),
        ]),
        execution_policy: Cow::Borrowed(b"pinned-numpyro-lkj-coregionalized-multitype-lgcp-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind:
            "application/vnd.marklab.numpyro-correlated-replicated-multitype-lgcp+json;version=2",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            correlated_replicated_arbitrary_window_multitype_lgcp::execute(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &Output| prepared.validate_result(output).map_err(NodeError::decode),
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.correlated_multitype_lgcp", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "correlated-replicated-arbitrary-window-multitype-lgcp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
