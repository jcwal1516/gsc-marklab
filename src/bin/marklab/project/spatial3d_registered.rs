use marklab_workflow::NodeError;

use super::super::spatial3d_registered::{
    self, PreparedRegisteredSerialVoxelK, RegisteredSerialVoxelKResult,
};
use super::cli_schema::Spatial3dRegisteredSerialVoxelKProjectArgs;
use super::{
    durable::{run_native_request, Codec, NativeSpecCommand},
    CliError,
};

const COMMAND: NativeSpecCommand = NativeSpecCommand {
    name: "spatial3d-registered-serial-voxel-k",
    label: "registered serial voxel K input",
    input_kind: "application/vnd.marklab.source.spatial3d-registered-serial-voxel-k+json;version=1",
    node_id: "spatial3d-registered-serial-voxel-k",
    node_kind: "spatial3d_registered_serial_voxel_k",
    node_version: 1,
    implementation_identity: "marklab-project-spatial3d-registered-serial-voxel-k-node-v2",
    execution_policy: b"native-safe-rust-registered-serial-voxel-k-v2",
    output_kind: "application/vnd.marklab.spatial3d-registered-serial-voxel-k+json;version=2",
    result_schema: ("marklab.spatial3d_registered_serial_voxel_k", 2),
    codec: Codec::PrettyJson,
};

pub(super) fn run(args: Spatial3dRegisteredSerialVoxelKProjectArgs) -> Result<(), CliError> {
    let Spatial3dRegisteredSerialVoxelKProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_request(
        &COMMAND,
        &project_path,
        input_path,
        &output_path,
        |bytes| {
            spatial3d_registered::prepare(bytes).map_err(|error| CliError::Input(error.to_string()))
        },
        |prepared: &PreparedRegisteredSerialVoxelK| {
            spatial3d_registered::execute(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &RegisteredSerialVoxelKResult| {
            output
                .validate_for_prepared(prepared)
                .map_err(NodeError::decode)
        },
    )
}
