use std::borrow::Cow;

use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::super::bayes::replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::{
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
    "marklab-project-numpyro-replicated-arbitrary-window-multitype-lgcp-sbc-node-v1";

pub(super) type ProjectArgs = InProject<Args>;

pub(super) fn run(arguments: ProjectArgs) -> Result<(), CliError> {
    const LABEL: &str = "multitype SBC source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = arguments;
    let source = Source::identify(arguments.input.clone(), INPUT_KIND)?;
    let output_path = std::mem::take(&mut arguments.out);
    let prepared =
        replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::prepare(arguments)?;
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "replicated-arbitrary-window-multitype-lgcp-sbc",
        kind: "bayesian_multitype_point_process_simulation_calibration",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            prepared.request_bytes(),
        ]),
        execution_policy: Cow::Borrowed(b"pinned-numpyro-five-scenario-multitype-lgcp-sbc-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind: "application/vnd.marklab.numpyro-replicated-arbitrary-window-multitype-lgcp-sbc+json;version=1",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::execute(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &Output| prepared.validate_result(output).map_err(NodeError::decode),
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.multitype_lgcp_sbc", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-sbc",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
