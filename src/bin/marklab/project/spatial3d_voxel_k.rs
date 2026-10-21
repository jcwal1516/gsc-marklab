use marklab_spatial3d::{voxel_window_k3d, VoxelWindowK3dResult, VoxelWindowK3dSpec};
use marklab_workflow::NodeError;

use super::cli_schema::Spatial3dVoxelKFunctionProjectArgs;
use super::{
    durable::{run_native_spec, Codec, NativeSpecCommand},
    CliError,
};

const COMMAND: NativeSpecCommand = NativeSpecCommand {
    name: "spatial3d-voxel-k-function",
    label: "spatial3d voxel-window K input",
    input_kind: "application/vnd.marklab.source.spatial3d-voxel-k-function+json;version=1",
    node_id: "spatial3d-voxel-k-function",
    node_kind: "spatial3d_voxel_window_homogeneous_k",
    node_version: 1,
    implementation_identity: "marklab-project-spatial3d-voxel-k-function-node-v2",
    execution_policy: b"native-safe-rust-bounded-physical-voxel-window-k3d-v2",
    output_kind: "application/vnd.marklab.spatial3d-voxel-k-function+json;version=2",
    result_schema: ("marklab.spatial3d_voxel_window_homogeneous_k", 2),
    codec: Codec::PrettyJson,
};

pub(super) fn run(args: Spatial3dVoxelKFunctionProjectArgs) -> Result<(), CliError> {
    let Spatial3dVoxelKFunctionProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &COMMAND,
        &project_path,
        input_path,
        &output_path,
        |analysis: &VoxelWindowK3dSpec| {
            voxel_window_k3d(analysis.clone()).map_err(NodeError::execution)
        },
        |analysis, output: &VoxelWindowK3dResult| {
            output
                .validate_for_spec(analysis)
                .map_err(NodeError::decode)
        },
    )
}
