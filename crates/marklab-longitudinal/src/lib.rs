#![forbid(unsafe_code)]
//! Bounded linear-Gaussian longitudinal inference for Marklab.

mod matrix;
mod nonlinear;
mod particle;
mod phylogenetic_association;

pub use nonlinear::{
    nonlinear_gaussian_filter, NonlinearFilterMethod, NonlinearFilterStep,
    NonlinearStepDiagnostics, QuadraticFunction, ScalarMoment, ScalarNonlinearFilterResult,
    ScalarNonlinearFilterSpec,
};
pub use particle::{
    particle_filter_and_smooth, ParticleFilterStepSpec, ParticleFilteringStep,
    ScalarParticleSmootherResult, ScalarParticleSmootherSpec, SmoothedParticleTrajectory,
};
pub use phylogenetic_association::{
    phylogenetic_spatial_association, CloneSpatialRecord, PhylogeneticSpatialAssociationResult,
    PhylogeneticSpatialAssociationSpec, PhylogeneticSpatialPair, TreeEdge,
};

use matrix::Matrix;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinearGaussianStateSpaceSpec {
    pub observations: Vec<Vec<Option<f64>>>,
    pub transition_matrices: Vec<Vec<Vec<f64>>>,
    pub process_covariances: Vec<Vec<Vec<f64>>>,
    pub observation_matrices: Vec<Vec<Vec<f64>>>,
    pub observation_covariances: Vec<Vec<Vec<f64>>>,
    pub initial_mean: Vec<f64>,
    pub initial_covariance: Vec<Vec<f64>>,
    pub maximum_time_steps: usize,
    pub maximum_state_dimension: usize,
    pub maximum_observation_dimension: usize,
    pub maximum_matrix_operations: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateEstimate {
    pub time_index: usize,
    pub mean: Vec<f64>,
    pub covariance: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LinearGaussianStateSpaceResult {
    pub format: &'static str,
    pub version: u32,
    pub algorithm: &'static str,
    pub state_dimension: usize,
    pub time_steps: usize,
    pub observed_components_per_step: Vec<usize>,
    pub observed_updates: usize,
    pub log_likelihood: f64,
    pub predicted_states: Vec<StateEstimate>,
    pub filtered_states: Vec<StateEstimate>,
    pub smoothed_states: Vec<StateEstimate>,
    pub planned_matrix_operations_upper_bound: u64,
    pub maximum_matrix_operations: u64,
    pub claim_status: &'static str,
}

impl<'de> Deserialize<'de> for LinearGaussianStateSpaceResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct OwnedResult {
            format: String,
            version: u32,
            algorithm: String,
            state_dimension: usize,
            time_steps: usize,
            observed_components_per_step: Vec<usize>,
            observed_updates: usize,
            log_likelihood: f64,
            predicted_states: Vec<StateEstimate>,
            filtered_states: Vec<StateEstimate>,
            smoothed_states: Vec<StateEstimate>,
            planned_matrix_operations_upper_bound: u64,
            maximum_matrix_operations: u64,
            claim_status: String,
        }

        let owned = OwnedResult::deserialize(deserializer)?;
        if owned.format != "marklab.linear_gaussian_state_space"
            || owned.algorithm != "kalman_joseph_rts_cholesky"
            || owned.claim_status != "linear_gaussian_model_only"
        {
            return Err(serde::de::Error::custom(
                "unexpected linear-Gaussian state-space result identity",
            ));
        }
        Ok(Self {
            format: "marklab.linear_gaussian_state_space",
            version: owned.version,
            algorithm: "kalman_joseph_rts_cholesky",
            state_dimension: owned.state_dimension,
            time_steps: owned.time_steps,
            observed_components_per_step: owned.observed_components_per_step,
            observed_updates: owned.observed_updates,
            log_likelihood: owned.log_likelihood,
            predicted_states: owned.predicted_states,
            filtered_states: owned.filtered_states,
            smoothed_states: owned.smoothed_states,
            planned_matrix_operations_upper_bound: owned.planned_matrix_operations_upper_bound,
            maximum_matrix_operations: owned.maximum_matrix_operations,
            claim_status: "linear_gaussian_model_only",
        })
    }
}

#[derive(Debug, Error)]
pub enum LongitudinalError {
    #[error("invalid longitudinal specification: {0}")]
    Invalid(String),
    #[error("longitudinal numerical failure: {0}")]
    Numerical(String),
    #[error("longitudinal resource limit exceeded: {0}")]
    Resource(String),
}

struct ValidatedStep {
    observation: Vec<Option<f64>>,
    transition: Matrix,
    process_covariance: Matrix,
    observation_matrix: Matrix,
    observation_covariance: Matrix,
}

struct ValidatedModel {
    initial_mean: Vec<f64>,
    initial_covariance: Matrix,
    steps: Vec<ValidatedStep>,
    planned_operations: u64,
    maximum_operations: u64,
}

pub fn kalman_filter_and_smooth(
    spec: LinearGaussianStateSpaceSpec,
) -> Result<LinearGaussianStateSpaceResult, LongitudinalError> {
    let model = validate(spec)?;
    let state_dimension = model.initial_mean.len();
    let mut prior_mean = model.initial_mean;
    let mut prior_covariance = model.initial_covariance;
    let mut predicted = Vec::with_capacity(model.steps.len());
    let mut filtered = Vec::with_capacity(model.steps.len());
    let mut observed_counts = Vec::with_capacity(model.steps.len());
    let mut log_likelihood = 0.0;

    for (time_index, step) in model.steps.iter().enumerate() {
        let predicted_mean = step.transition.mul_vec(&prior_mean);
        let predicted_covariance = step
            .transition
            .mul(&prior_covariance)
            .mul(&step.transition.transpose())
            .add(&step.process_covariance)
            .symmetrized();
        ensure_finite(&predicted_mean, &predicted_covariance, "predicted state")?;
        let observed = step
            .observation
            .iter()
            .enumerate()
            .filter_map(|(index, value)| value.map(|_| index))
            .collect::<Vec<_>>();
        observed_counts.push(observed.len());
        let (filtered_mean, filtered_covariance, likelihood) = if observed.is_empty() {
            (predicted_mean.clone(), predicted_covariance.clone(), 0.0)
        } else {
            update(step, &observed, &predicted_mean, &predicted_covariance)?
        };
        log_likelihood += likelihood;
        predicted.push(estimate(time_index, predicted_mean, predicted_covariance));
        prior_mean = filtered_mean.clone();
        prior_covariance = filtered_covariance.clone();
        filtered.push(estimate(time_index, filtered_mean, filtered_covariance));
    }

    let mut smoothed = filtered.clone();
    for time_index in (0..model.steps.len().saturating_sub(1)).rev() {
        let next_transition = &model.steps[time_index + 1].transition;
        let filtered_covariance = matrix_from_estimate(&filtered[time_index], state_dimension)?;
        let next_predicted_covariance =
            matrix_from_estimate(&predicted[time_index + 1], state_dimension)?;
        let smoother_rhs = next_transition.mul(&filtered_covariance).transpose();
        let solved = next_predicted_covariance
            .cholesky("predicted covariance during RTS smoothing")?
            .solve_matrix(&smoother_rhs);
        let gain = solved.transpose();
        let mean_delta = subtract(
            &smoothed[time_index + 1].mean,
            &predicted[time_index + 1].mean,
        );
        let smooth_mean = add(&filtered[time_index].mean, &gain.mul_vec(&mean_delta));
        let smooth_next_covariance =
            matrix_from_estimate(&smoothed[time_index + 1], state_dimension)?;
        let covariance_delta = smooth_next_covariance.sub(&next_predicted_covariance);
        let smooth_covariance = filtered_covariance
            .add(&gain.mul(&covariance_delta).mul(&gain.transpose()))
            .symmetrized();
        ensure_finite(&smooth_mean, &smooth_covariance, "smoothed state")?;
        smoothed[time_index] = estimate(time_index, smooth_mean, smooth_covariance);
    }
    if !log_likelihood.is_finite() {
        return Err(LongitudinalError::Numerical(
            "innovation log likelihood is not finite".into(),
        ));
    }
    let observed_updates = observed_counts.iter().filter(|&&count| count > 0).count();
    Ok(LinearGaussianStateSpaceResult {
        format: "marklab.linear_gaussian_state_space",
        version: 1,
        algorithm: "kalman_joseph_rts_cholesky",
        state_dimension,
        time_steps: model.steps.len(),
        observed_components_per_step: observed_counts,
        observed_updates,
        log_likelihood,
        predicted_states: predicted,
        filtered_states: filtered,
        smoothed_states: smoothed,
        planned_matrix_operations_upper_bound: model.planned_operations,
        maximum_matrix_operations: model.maximum_operations,
        claim_status: "linear_gaussian_model_only",
    })
}

fn update(
    step: &ValidatedStep,
    observed: &[usize],
    predicted_mean: &[f64],
    predicted_covariance: &Matrix,
) -> Result<(Vec<f64>, Matrix, f64), LongitudinalError> {
    let observation_matrix = step.observation_matrix.selected_rows(observed);
    let observation_covariance = step.observation_covariance.principal_submatrix(observed);
    let observed_values = observed
        .iter()
        .map(|&index| step.observation[index].expect("observed indices contain values"))
        .collect::<Vec<_>>();
    let innovation = subtract(
        &observed_values,
        &observation_matrix.mul_vec(predicted_mean),
    );
    let innovation_covariance = observation_matrix
        .mul(predicted_covariance)
        .mul(&observation_matrix.transpose())
        .add(&observation_covariance)
        .symmetrized();
    let factor = innovation_covariance.cholesky("innovation covariance")?;
    let solved_innovation = factor.solve_vector(&innovation);
    let gain_rhs = observation_matrix.mul(predicted_covariance);
    let gain = factor.solve_matrix(&gain_rhs).transpose();
    let filtered_mean = add(predicted_mean, &gain.mul_vec(&innovation));
    let identity_minus_gain_h =
        Matrix::identity(predicted_covariance.rows()).sub(&gain.mul(&observation_matrix));
    let filtered_covariance = identity_minus_gain_h
        .mul(predicted_covariance)
        .mul(&identity_minus_gain_h.transpose())
        .add(&gain.mul(&observation_covariance).mul(&gain.transpose()))
        .symmetrized();
    ensure_finite(&filtered_mean, &filtered_covariance, "filtered state")?;
    let quadratic = innovation
        .iter()
        .zip(solved_innovation)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    let likelihood = -0.5
        * (observed.len() as f64 * (2.0 * std::f64::consts::PI).ln()
            + factor.log_determinant()
            + quadratic);
    Ok((filtered_mean, filtered_covariance, likelihood))
}

fn validate(spec: LinearGaussianStateSpaceSpec) -> Result<ValidatedModel, LongitudinalError> {
    let time_steps = spec.observations.len();
    let state_dimension = spec.initial_mean.len();
    if time_steps == 0 || time_steps > spec.maximum_time_steps {
        return Err(LongitudinalError::Resource(format!(
            "time steps {time_steps} must be positive and no greater than {}",
            spec.maximum_time_steps
        )));
    }
    if state_dimension == 0 || state_dimension > spec.maximum_state_dimension {
        return Err(LongitudinalError::Resource(format!(
            "state dimension {state_dimension} must be positive and no greater than {}",
            spec.maximum_state_dimension
        )));
    }
    if spec.maximum_observation_dimension == 0 || spec.maximum_matrix_operations == 0 {
        return Err(LongitudinalError::Resource(
            "observation and matrix-operation limits must be positive".into(),
        ));
    }
    if spec.initial_mean.iter().any(|value| !value.is_finite()) {
        return Err(LongitudinalError::Invalid(
            "initial mean must contain only finite values".into(),
        ));
    }
    let sequence_lengths = [
        spec.transition_matrices.len(),
        spec.process_covariances.len(),
        spec.observation_matrices.len(),
        spec.observation_covariances.len(),
    ];
    if sequence_lengths.iter().any(|&length| length != time_steps) {
        return Err(LongitudinalError::Invalid(
            "every matrix sequence must have exactly one entry per observation step".into(),
        ));
    }
    let initial_covariance = Matrix::from_rows(
        &spec.initial_covariance,
        state_dimension,
        state_dimension,
        "initial covariance",
    )?;
    initial_covariance.validate_symmetric_positive_definite("initial covariance")?;
    let mut steps = Vec::with_capacity(time_steps);
    let mut planned_operations = 0_u64;
    for time_index in 0..time_steps {
        let observation_dimension = spec.observations[time_index].len();
        if observation_dimension == 0 || observation_dimension > spec.maximum_observation_dimension
        {
            return Err(LongitudinalError::Resource(format!(
                "observation dimension at step {time_index} must be positive and no greater than {}",
                spec.maximum_observation_dimension
            )));
        }
        if spec.observations[time_index]
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(LongitudinalError::Invalid(format!(
                "observation at step {time_index} contains a non-finite value"
            )));
        }
        let transition = Matrix::from_rows(
            &spec.transition_matrices[time_index],
            state_dimension,
            state_dimension,
            "transition matrix",
        )?;
        let process_covariance = Matrix::from_rows(
            &spec.process_covariances[time_index],
            state_dimension,
            state_dimension,
            "process covariance",
        )?;
        process_covariance.validate_symmetric_positive_semidefinite("process covariance")?;
        let observation_matrix = Matrix::from_rows(
            &spec.observation_matrices[time_index],
            observation_dimension,
            state_dimension,
            "observation matrix",
        )?;
        let observation_covariance = Matrix::from_rows(
            &spec.observation_covariances[time_index],
            observation_dimension,
            observation_dimension,
            "observation covariance",
        )?;
        observation_covariance.validate_symmetric_positive_definite("observation covariance")?;
        let n = u64::try_from(state_dimension).map_err(|_| {
            LongitudinalError::Resource("state dimension cannot be represented".into())
        })?;
        let m = u64::try_from(observation_dimension).map_err(|_| {
            LongitudinalError::Resource("observation dimension cannot be represented".into())
        })?;
        let step_operations = n
            .checked_pow(3)
            .and_then(|value| value.checked_mul(20))
            .and_then(|value| value.checked_add(m.checked_pow(3)?.checked_mul(10)?))
            .and_then(|value| value.checked_add(n.checked_mul(n)?.checked_mul(m)?.checked_mul(20)?))
            .ok_or_else(|| LongitudinalError::Resource("matrix work plan overflowed".into()))?;
        planned_operations = planned_operations
            .checked_add(step_operations)
            .ok_or_else(|| LongitudinalError::Resource("matrix work plan overflowed".into()))?;
        steps.push(ValidatedStep {
            observation: spec.observations[time_index].clone(),
            transition,
            process_covariance,
            observation_matrix,
            observation_covariance,
        });
    }
    if planned_operations > spec.maximum_matrix_operations {
        return Err(LongitudinalError::Resource(format!(
            "planned matrix-operation upper bound {planned_operations} exceeds declared maximum {}",
            spec.maximum_matrix_operations
        )));
    }
    Ok(ValidatedModel {
        initial_mean: spec.initial_mean,
        initial_covariance,
        steps,
        planned_operations,
        maximum_operations: spec.maximum_matrix_operations,
    })
}

fn estimate(time_index: usize, mean: Vec<f64>, covariance: Matrix) -> StateEstimate {
    StateEstimate {
        time_index,
        mean,
        covariance: covariance.into_rows(),
    }
}

fn matrix_from_estimate(
    estimate: &StateEstimate,
    dimension: usize,
) -> Result<Matrix, LongitudinalError> {
    Matrix::from_rows(
        &estimate.covariance,
        dimension,
        dimension,
        "state covariance",
    )
}

fn add(left: &[f64], right: &[f64]) -> Vec<f64> {
    left.iter().zip(right).map(|(a, b)| a + b).collect()
}

fn subtract(left: &[f64], right: &[f64]) -> Vec<f64> {
    left.iter().zip(right).map(|(a, b)| a - b).collect()
}

fn ensure_finite(mean: &[f64], covariance: &Matrix, name: &str) -> Result<(), LongitudinalError> {
    if mean.iter().any(|value| !value.is_finite())
        || covariance
            .clone()
            .into_rows()
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
    {
        return Err(LongitudinalError::Numerical(format!(
            "{name} contains a non-finite value"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_and_fully_missing_multivariate_observations_are_explicit() {
        let result = kalman_filter_and_smooth(LinearGaussianStateSpaceSpec {
            observations: vec![vec![Some(1.0), None], vec![None, None]],
            transition_matrices: vec![identity_rows(2), identity_rows(2)],
            process_covariances: vec![diagonal_rows(2, 0.1), diagonal_rows(2, 0.1)],
            observation_matrices: vec![identity_rows(2), identity_rows(2)],
            observation_covariances: vec![diagonal_rows(2, 1.0), diagonal_rows(2, 1.0)],
            initial_mean: vec![0.0, 5.0],
            initial_covariance: diagonal_rows(2, 1.0),
            maximum_time_steps: 2,
            maximum_state_dimension: 2,
            maximum_observation_dimension: 2,
            maximum_matrix_operations: 100_000,
        })
        .unwrap();
        assert_eq!(result.observed_components_per_step, vec![1, 0]);
        assert_eq!(result.observed_updates, 1);
        assert!((result.filtered_states[0].mean[0] - 1.1 / 2.1).abs() < 1.0e-12);
        assert_eq!(result.filtered_states[0].mean[1], 5.0);
        assert_eq!(
            result.predicted_states[1].mean,
            result.filtered_states[1].mean
        );
        assert_eq!(
            result.predicted_states[1].covariance,
            result.filtered_states[1].covariance
        );
    }

    #[test]
    fn indefinite_declared_covariance_is_rejected() {
        let error = kalman_filter_and_smooth(LinearGaussianStateSpaceSpec {
            observations: vec![vec![Some(1.0)]],
            transition_matrices: vec![identity_rows(1)],
            process_covariances: vec![vec![vec![-1.0]]],
            observation_matrices: vec![identity_rows(1)],
            observation_covariances: vec![identity_rows(1)],
            initial_mean: vec![0.0],
            initial_covariance: identity_rows(1),
            maximum_time_steps: 1,
            maximum_state_dimension: 1,
            maximum_observation_dimension: 1,
            maximum_matrix_operations: 1_000,
        })
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("process covariance is not positive semidefinite"));
    }

    fn identity_rows(size: usize) -> Vec<Vec<f64>> {
        (0..size)
            .map(|row| (0..size).map(|col| f64::from(row == col)).collect())
            .collect()
    }

    fn diagonal_rows(size: usize, diagonal: f64) -> Vec<Vec<f64>> {
        (0..size)
            .map(|row| {
                (0..size)
                    .map(|col| if row == col { diagonal } else { 0.0 })
                    .collect()
            })
            .collect()
    }
}
