use marklab_graph::{
    typed_triangle_motif_summary_workflow, TypedTriangleMotifSpec, TypedTriangleMotifSummaryResult,
};
use marklab_workflow::NodeError;

use super::cli_schema::GraphMotifTriangleSummaryProjectArgs;
use super::{
    durable::{run_native_spec, Codec, NativeSpecCommand},
    CliError,
};

const COMMAND: NativeSpecCommand = NativeSpecCommand {
    name: "graph-motif-triangle-summary",
    label: "graph motif summary input",
    input_kind: "application/vnd.marklab.source.graph-motif-triangle-summary+json;version=1",
    node_id: "graph-motif-triangle-summary",
    node_kind: "graph_typed_triangle_motif_summary",
    node_version: 1,
    implementation_identity: "marklab-project-graph-motif-triangle-summary-node-v1",
    execution_policy: b"native-safe-rust-graph-motif-triangle-summary-v1",
    output_kind: "application/vnd.marklab.graph-motif-triangle-summary+json;version=1",
    result_schema: ("marklab.graph_motif_triangle_summary", 1),
    codec: Codec::ExactFloatJson,
};

pub(super) fn run(args: GraphMotifTriangleSummaryProjectArgs) -> Result<(), CliError> {
    let GraphMotifTriangleSummaryProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &COMMAND,
        &project_path,
        input_path,
        &output_path,
        |spec: &TypedTriangleMotifSpec| {
            typed_triangle_motif_summary_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &TypedTriangleMotifSummaryResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}
