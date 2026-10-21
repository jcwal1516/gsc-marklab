use std::borrow::Cow;

use marklab_bayes::{fit_sparse_car, SparseCarError, SparseCarResult};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::SparseCarFitProjectArgs;
use super::{
    bayes,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.sparse-car-spec+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-sparse-car-fit-node-v1";

pub(super) fn run(args: SparseCarFitProjectArgs) -> Result<(), CliError> {
    let SparseCarFitProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    const LABEL: &str = "sparse CAR input";
    let (spec, request_bytes) = bayes::sparse_car::read_spec(&input_path)?;
    spec.validate().map_err(map_error)?;
    let source = Source::from_bytes(input_path, INPUT_KIND, &request_bytes)?;
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "sparse-car-fit",
        kind: "bayesian_sparse_car_fit",
        version: 1,
        configuration_digest: ContentDigest::from_framed([IMPLEMENTATION_IDENTITY.as_bytes()]),
        execution_policy: Cow::Borrowed(b"native-safe-rust-sparse-car-fit-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind: "application/vnd.marklab.sparse-car-fit+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        spec,
        |spec| fit_sparse_car(spec).map_err(NodeError::execution),
        |spec, output: &SparseCarResult| output.validate_for(spec).map_err(NodeError::decode),
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.sparse_car_fit", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "sparse-car-fit",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn map_error(error: SparseCarError) -> CliError {
    match error {
        SparseCarError::InvalidInput(message) | SparseCarError::ResourceLimit(message) => {
            CliError::Input(message)
        }
        SparseCarError::Numerical(message) => CliError::Backend(message),
    }
}
