use marklab_longitudinal::{
    kalman_filter_and_smooth, LinearGaussianStateSpaceResult, LinearGaussianStateSpaceSpec,
};
use marklab_workflow::NodeError;

use super::cli_schema::LongitudinalKalmanSmoothProjectArgs;
use super::{
    durable::{run_native_spec, Codec, NativeSpecCommand},
    CliError,
};

const COMMAND: NativeSpecCommand = NativeSpecCommand {
    name: "longitudinal-kalman-smooth",
    label: "longitudinal Kalman input",
    input_kind:
        "application/vnd.marklab.source.longitudinal-linear-gaussian-state-space+json;version=1",
    node_id: "longitudinal-kalman-smooth",
    node_kind: "longitudinal_linear_gaussian_kalman_rts",
    node_version: 1,
    implementation_identity: "marklab-project-longitudinal-kalman-rts-node-v2",
    execution_policy: b"native-safe-rust-bounded-kalman-joseph-rts-v1",
    output_kind: "application/vnd.marklab.longitudinal-linear-gaussian-state-space+json;version=1",
    result_schema: ("marklab.longitudinal_linear_gaussian_state_space", 1),
    codec: Codec::PrettyJson,
};

pub(super) fn run(args: LongitudinalKalmanSmoothProjectArgs) -> Result<(), CliError> {
    let LongitudinalKalmanSmoothProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &COMMAND,
        &project_path,
        input_path,
        &output_path,
        |analysis: &LinearGaussianStateSpaceSpec| {
            kalman_filter_and_smooth(analysis.clone()).map_err(NodeError::execution)
        },
        |analysis, output| {
            validate_output(analysis, output)
                .map_err(|message| NodeError::decode(CliError::Backend(message)))
        },
    )
}

fn validate_output(
    analysis: &LinearGaussianStateSpaceSpec,
    output: &LinearGaussianStateSpaceResult,
) -> Result<(), String> {
    let expected_steps = analysis.observations.len();
    let state_dimension = analysis.initial_mean.len();
    let state_valid = |state: &marklab_longitudinal::StateEstimate| {
        state.time_index < expected_steps
            && state.mean.len() == state_dimension
            && state.covariance.len() == state_dimension
            && state.covariance.iter().all(|row| {
                row.len() == state_dimension && row.iter().all(|value| value.is_finite())
            })
            && state.mean.iter().all(|value| value.is_finite())
    };
    if output.version != 1
        || output.state_dimension != state_dimension
        || output.time_steps != expected_steps
        || output.observed_components_per_step.len() != expected_steps
        || output.predicted_states.len() != expected_steps
        || output.filtered_states.len() != expected_steps
        || output.smoothed_states.len() != expected_steps
        || output.maximum_matrix_operations != analysis.maximum_matrix_operations
        || output.planned_matrix_operations_upper_bound > output.maximum_matrix_operations
        || !output.log_likelihood.is_finite()
        || !output.predicted_states.iter().all(state_valid)
        || !output.filtered_states.iter().all(state_valid)
        || !output.smoothed_states.iter().all(state_valid)
    {
        return Err(
            "longitudinal Kalman cached result identity, dimensions, bounds, or finite policy differs"
                .into(),
        );
    }
    Ok(())
}
