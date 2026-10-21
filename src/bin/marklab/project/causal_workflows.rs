use std::collections::HashSet;

use marklab_causal::{
    estimate_gaussian_expected_information_gain, randomized_binary_interference, GaussianEigResult,
    GaussianEigSpec, RandomizedInterferenceResult, RandomizedInterferenceSpec,
};
use marklab_workflow::NodeError;

use super::cli_schema::{CausalRandomizedInterferenceProjectArgs, GaussianEigProjectArgs};
use super::{
    durable::{run_native_spec, Codec, NativeSpecCommand},
    CliError,
};

const RANDOMIZED_INTERFERENCE: NativeSpecCommand = NativeSpecCommand {
    name: "causal-randomized-interference",
    label: "causal randomized-interference input",
    input_kind: "application/vnd.marklab.source.causal-randomized-interference+json;version=1",
    node_id: "causal-randomized-interference",
    node_kind: "causal_randomized_binary_interference",
    node_version: 1,
    implementation_identity: "marklab-project-causal-randomized-interference-node-v1",
    execution_policy: b"native-safe-rust-bounded-randomized-interference-v1",
    output_kind: "application/vnd.marklab.causal-randomized-interference+json;version=1",
    result_schema: ("marklab.causal_randomized_interference", 1),
    codec: Codec::Json,
};

const GAUSSIAN_EIG: NativeSpecCommand = NativeSpecCommand {
    name: "gaussian-eig",
    label: "Gaussian EIG input",
    input_kind: "application/vnd.marklab.source.gaussian-eig+json;version=1",
    node_id: "gaussian-eig",
    node_kind: "causal_gaussian_expected_information_gain",
    node_version: 1,
    implementation_identity: "marklab-project-gaussian-eig-node-v1",
    execution_policy: b"native-safe-rust-bounded-gaussian-eig-v1",
    output_kind: "application/vnd.marklab.gaussian-eig+json;version=1",
    result_schema: ("marklab.gaussian_expected_information_gain", 1),
    codec: Codec::Json,
};

pub(super) fn run_randomized_interference(
    args: CausalRandomizedInterferenceProjectArgs,
) -> Result<(), CliError> {
    let CausalRandomizedInterferenceProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &RANDOMIZED_INTERFERENCE,
        &project_path,
        input_path,
        &output_path,
        |analysis: &RandomizedInterferenceSpec| {
            randomized_binary_interference(analysis.clone()).map_err(NodeError::execution)
        },
        |analysis, output| {
            validate_interference(analysis, output)
                .map_err(|message| NodeError::decode(CliError::Backend(message)))
        },
    )
}

pub(super) fn run_gaussian_eig(args: GaussianEigProjectArgs) -> Result<(), CliError> {
    let GaussianEigProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &GAUSSIAN_EIG,
        &project_path,
        input_path,
        &output_path,
        |analysis: &GaussianEigSpec| {
            estimate_gaussian_expected_information_gain(analysis.clone())
                .map_err(NodeError::execution)
        },
        |analysis, output| {
            validate_eig(analysis, output)
                .map_err(|message| NodeError::decode(CliError::Backend(message)))
        },
    )
}

fn validate_interference(
    analysis: &RandomizedInterferenceSpec,
    output: &RandomizedInterferenceResult,
) -> Result<(), String> {
    let clusters = analysis
        .units
        .iter()
        .map(|unit| unit.cluster_id.as_str())
        .collect::<HashSet<_>>();
    let probabilities_finite = output.exposure_probabilities.iter().all(|row| {
        let values = &row.probabilities;
        [
            values.untreated_neighbor_untreated,
            values.untreated_neighbor_treated,
            values.treated_neighbor_untreated,
            values.treated_neighbor_treated,
        ]
        .into_iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
    });
    if output.version != 1
        || output.unit_count != analysis.units.len()
        || output.cluster_count != clusters.len()
        || output.design_provenance != analysis.design_provenance
        || output.graph_provenance != analysis.graph_provenance
        || output.assignment_states > analysis.maximum_assignment_states
        || output.unit_assignment_evaluations > analysis.maximum_unit_assignment_evaluations
        || output.observed_exposures.len() != analysis.units.len()
        || output.exposure_probabilities.len() != analysis.units.len()
        || output.randomization_test.high != analysis.test_exposure_high
        || output.randomization_test.low != analysis.test_exposure_low
        || output.randomization_test.null_values.len() != analysis.permutations
        || !output.randomization_test.observed.is_finite()
        || !output.randomization_test.p_value.is_finite()
        || !(0.0..=1.0).contains(&output.randomization_test.p_value)
        || !probabilities_finite
    {
        return Err(
            "randomized-interference cached result identity, bounds, or finite policy differs"
                .into(),
        );
    }
    Ok(())
}

fn validate_eig(analysis: &GaussianEigSpec, output: &GaussianEigResult) -> Result<(), String> {
    let expected_work = (analysis.outer_samples as u64)
        .checked_mul(analysis.inner_samples as u64 + 1)
        .ok_or_else(|| "Gaussian EIG work identity overflows".to_owned())?;
    let analytic = 0.5
        * (1.0
            + analysis.sensitivity.powi(2) * analysis.prior_sd.powi(2) / analysis.noise_sd.powi(2))
        .ln();
    let finite = [
        output.estimate,
        output.monte_carlo_se,
        output.analytic_eig,
        output.signed_bias_against_analytic,
        output.absolute_bias_against_analytic,
    ]
    .into_iter()
    .all(f64::is_finite)
        && output.outer_values.iter().all(|row| {
            [
                row.theta,
                row.simulated_observation,
                row.log_numerator,
                row.log_denominator,
                row.information_value,
            ]
            .into_iter()
            .all(f64::is_finite)
        });
    if output.version != 1
        || output.candidate_id != analysis.candidate_id
        || output.outer_samples != analysis.outer_samples
        || output.inner_samples != analysis.inner_samples
        || output.outer_values.len() != analysis.outer_samples
        || !output
            .outer_values
            .iter()
            .enumerate()
            .all(|(index, row)| row.outer_index == index)
        || output.likelihood_evaluations != expected_work
        || output.likelihood_evaluations > analysis.maximum_likelihood_evaluations
        || (output.analytic_eig - analytic).abs() > 1.0e-15
        || !finite
    {
        return Err("Gaussian EIG cached result identity, bounds, or finite policy differs".into());
    }
    Ok(())
}
