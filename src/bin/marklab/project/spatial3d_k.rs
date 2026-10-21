use marklab_spatial3d::{homogeneous_k3d, HomogeneousK3dResult, HomogeneousK3dSpec};
use marklab_workflow::NodeError;

use super::cli_schema::Spatial3dKFunctionProjectArgs;
use super::{
    durable::{run_native_spec, Codec, NativeSpecCommand},
    CliError,
};

const COMMAND: NativeSpecCommand = NativeSpecCommand {
    name: "spatial3d-k-function",
    label: "spatial3d K input",
    input_kind: "application/vnd.marklab.source.spatial3d-k-function+json;version=1",
    node_id: "spatial3d-k-function",
    node_kind: "spatial3d_homogeneous_k",
    node_version: 1,
    implementation_identity: "marklab-project-spatial3d-k-function-node-v2",
    execution_policy: b"native-safe-rust-bounded-physical-spatial3d-k-v2",
    output_kind: "application/vnd.marklab.spatial3d-k-function+json;version=2",
    result_schema: ("marklab.spatial3d_homogeneous_k", 2),
    codec: Codec::PrettyJson,
};

pub(super) fn run(args: Spatial3dKFunctionProjectArgs) -> Result<(), CliError> {
    let Spatial3dKFunctionProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &COMMAND,
        &project_path,
        input_path,
        &output_path,
        |analysis: &HomogeneousK3dSpec| {
            homogeneous_k3d(analysis.clone()).map_err(NodeError::execution)
        },
        |analysis, output| {
            validate_output(analysis, output)
                .map_err(|message| NodeError::decode(CliError::Backend(message)))
        },
    )
}

fn validate_output(
    analysis: &HomogeneousK3dSpec,
    output: &HomogeneousK3dResult,
) -> Result<(), String> {
    let point_count = analysis.points.len() as u64;
    let expected_pairs = point_count
        .checked_mul(point_count.saturating_sub(1))
        .map(|pairs| pairs / 2)
        .ok_or_else(|| "3-D pair identity overflows".to_owned())?;
    let expected_work = expected_pairs
        .checked_mul(analysis.radii_um.len() as u64)
        .ok_or_else(|| "3-D pair-radius identity overflows".to_owned())?;
    let finite_curve = output.curve.iter().all(|row| {
        row.radius_um.is_finite()
            && row.k_um3.is_finite()
            && row.l3_um.is_finite()
            && row.k_um3 >= 0.0
    });
    if output.version != 2
        || output.dimension != 3
        || output.normalized_points.len() != analysis.points.len()
        || output.curve.len() != analysis.radii_um.len()
        || output.correction != analysis.correction
        || output.unordered_pairs_visited != expected_pairs
        || output.pair_radius_evaluations != expected_work
        || output.unordered_pairs_visited > analysis.maximum_unordered_pairs
        || !finite_curve
        || !output.window.volume_um3.is_finite()
        || output.window.volume_um3 <= 0.0
    {
        return Err("spatial3d K cached result identity, bounds, or finite policy differs".into());
    }
    Ok(())
}
