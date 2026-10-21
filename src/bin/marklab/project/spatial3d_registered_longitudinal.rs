use marklab_workflow::NodeError;

use super::super::spatial3d_registered::longitudinal::{
    self, PreparedRegisteredLongitudinalVoxelK, RegisteredLongitudinalVoxelKResult,
};
use super::cli_schema::Spatial3dRegisteredLongitudinalVoxelKChangeProjectArgs;
use super::{
    durable::{run_native_request, Codec, NativeSpecCommand},
    CliError,
};

const COMMAND: NativeSpecCommand = NativeSpecCommand {
    name: "spatial3d-registered-longitudinal-voxel-k-change",
    label: "registered longitudinal voxel K input",
    input_kind: "application/vnd.marklab.source.spatial3d-registered-longitudinal-voxel-k-change+json;version=1",
    node_id: "spatial3d-registered-longitudinal-voxel-k-change",
    node_kind: "spatial3d_registered_longitudinal_voxel_k_change",
    node_version: 1,
    implementation_identity: "marklab-project-spatial3d-registered-longitudinal-voxel-k-change-node-v1",
    execution_policy: b"native-safe-rust-registered-longitudinal-voxel-k-change-v1",
    output_kind: "application/vnd.marklab.spatial3d-registered-longitudinal-voxel-k-change+json;version=1",
    result_schema: ("marklab.spatial3d_registered_longitudinal_voxel_k_change", 1),
    codec: Codec::PrettyJson,
};

pub(super) fn run(
    args: Spatial3dRegisteredLongitudinalVoxelKChangeProjectArgs,
) -> Result<(), CliError> {
    let Spatial3dRegisteredLongitudinalVoxelKChangeProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_request(
        &COMMAND,
        &project_path,
        input_path,
        &output_path,
        |bytes| longitudinal::prepare(bytes).map_err(|error| CliError::Input(error.to_string())),
        |prepared: &PreparedRegisteredLongitudinalVoxelK| {
            longitudinal::execute(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &RegisteredLongitudinalVoxelKResult| {
            output
                .validate_for_prepared(prepared)
                .map_err(NodeError::decode)
        },
    )
}
