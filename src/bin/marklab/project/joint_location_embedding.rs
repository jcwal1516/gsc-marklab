use std::borrow::Cow;

use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::super::bayes::joint_replicated_location_embedding::{self, Args, Output};
use super::{
    cli_schema::InProject,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const LOCATION_KIND: &str =
    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1";
const EMBEDDING_KIND: &str =
    "application/vnd.marklab.source.replicated-projected-cell-embedding+csv;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-numpyro-joint-replicated-location-embedding-node-v2";

pub(super) type ProjectArgs = InProject<Args>;

pub(super) fn run(arguments: ProjectArgs) -> Result<(), CliError> {
    const LABEL: &str = "joint location-embedding sources";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = arguments;
    let sources = Source::identify_all(
        [
            arguments.location_input.clone(),
            arguments.embedding_input.clone(),
        ],
        &[LOCATION_KIND, EMBEDDING_KIND],
    )?;
    let output_path = std::mem::take(&mut arguments.out);
    let prepared = joint_replicated_location_embedding::prepare(arguments)?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = NodeContract {
        id: "joint-replicated-location-embedding",
        kind: "bayesian_joint_location_projected_embedding_fit",
        version: 2,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            prepared.request_bytes(),
        ]),
        execution_policy: Cow::Borrowed(
            b"pinned-numpyro-pattern-heldout-joint-location-embedding-v2",
        ),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind:
            "application/vnd.marklab.numpyro-joint-replicated-location-embedding+json;version=2",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| {
            joint_replicated_location_embedding::execute(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &Output| prepared.validate_result(output).map_err(NodeError::decode),
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.joint_replicated_location_embedding", 2),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "joint-replicated-location-embedding",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
