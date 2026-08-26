#![forbid(unsafe_code)]

mod posterior_predictive_lab;
mod sbc;
mod simulation_ood;
mod smc_abc;
mod synthetic_likelihood;

pub use posterior_predictive_lab::{
    growth_front_posterior_predictive_laboratory, GrowthFrontPosteriorPredictiveLabResult,
    GrowthFrontPosteriorPredictiveLabSpec, PosteriorPredictiveCheck, PosteriorPredictiveFailure,
    PosteriorPredictiveReplicate,
};
pub use sbc::{
    growth_front_rejection_abc_sbc, GrowthFrontRejectionAbcSbcResult,
    GrowthFrontRejectionAbcSbcSpec, SbcCoverage, SbcRankDiagnostics, SbcReplicate,
};
pub use simulation_ood::{
    detect_simulation_ood, SimulationOodCalibrationScore, SimulationOodObservation,
    SimulationOodResult, SimulationOodSpec, SimulationSummary,
};
pub use smc_abc::{
    smc_abc_growth_front, SmcAbcGrowthFrontResult, SmcAbcGrowthFrontSpec, SmcAbcParticle,
    SmcAbcStage,
};
pub use synthetic_likelihood::{
    synthetic_likelihood_growth_front, SyntheticLikelihoodEstimate,
    SyntheticLikelihoodGrowthFrontResult, SyntheticLikelihoodGrowthFrontSpec,
    SyntheticLikelihoodMcmcDiagnostics, SyntheticLikelihoodPosterior, SyntheticLikelihoodTraceRow,
};

use marklab_simulation::{
    simulate_growth_front, GrowthFrontInitialPoint, GrowthFrontSpec, SimulationError,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct RejectionAbcGrowthFrontSpec {
    pub initial: Vec<GrowthFrontInitialPoint>,
    pub diffusion_um2_per_time: f64,
    pub carrying_capacity: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub front_threshold_fraction: f64,
    pub observed_final_mass: f64,
    pub mass_scale: f64,
    pub growth_rate_prior_min: f64,
    pub growth_rate_prior_max: f64,
    pub epsilon: f64,
    pub accepted_draws: u32,
    pub maximum_proposals: u32,
    pub maximum_cell_steps_per_proposal: u64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RejectionAbcAcceptedDraw {
    pub proposal: u32,
    pub growth_rate_per_time: f64,
    pub simulated_final_mass: f64,
    pub standardized_mass_residual: f64,
    pub distance: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RejectionAbcPosterior {
    pub growth_rate_mean: f64,
    pub growth_rate_sd: f64,
    pub growth_rate_min: f64,
    pub growth_rate_max: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RejectionAbcGrowthFrontResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator: &'static str,
    pub summary: &'static str,
    pub distance: &'static str,
    pub prior: &'static str,
    pub epsilon: f64,
    pub accepted_draws_requested: u32,
    pub proposals_attempted: u32,
    pub acceptance_rate: f64,
    pub accepted: Vec<RejectionAbcAcceptedDraw>,
    pub posterior: RejectionAbcPosterior,
    pub maximum_total_declared_cell_steps: u64,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

#[derive(Debug, Error)]
pub enum SbiError {
    #[error("invalid SBI specification: {0}")]
    Invalid(String),
    #[error("SBI simulator failed: {0}")]
    Simulation(#[from] SimulationError),
    #[error("SBI acceptance target not reached: {0}")]
    Acceptance(String),
}

pub fn rejection_abc_growth_front(
    spec: RejectionAbcGrowthFrontSpec,
) -> Result<RejectionAbcGrowthFrontResult, SbiError> {
    let maximum_total_work = validate(&spec)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut accepted = Vec::with_capacity(spec.accepted_draws as usize);
    let mut proposals_attempted = 0_u32;
    while proposals_attempted < spec.maximum_proposals
        && accepted.len() < spec.accepted_draws as usize
    {
        proposals_attempted += 1;
        let growth_rate = rng.gen_range(spec.growth_rate_prior_min..spec.growth_rate_prior_max);
        let simulation = simulate_growth_front(GrowthFrontSpec {
            initial: spec.initial.clone(),
            diffusion_um2_per_time: spec.diffusion_um2_per_time,
            growth_rate_per_time: growth_rate,
            carrying_capacity: spec.carrying_capacity,
            final_time: spec.final_time,
            time_step: spec.time_step,
            front_threshold_fraction: spec.front_threshold_fraction,
            record_every_steps: u32::MAX,
            maximum_cell_steps: spec.maximum_cell_steps_per_proposal,
        })?;
        let standardized =
            (simulation.final_total_density_mass - spec.observed_final_mass) / spec.mass_scale;
        let distance = standardized.abs();
        if distance <= spec.epsilon {
            accepted.push(RejectionAbcAcceptedDraw {
                proposal: proposals_attempted,
                growth_rate_per_time: growth_rate,
                simulated_final_mass: simulation.final_total_density_mass,
                standardized_mass_residual: standardized,
                distance,
            });
        }
    }
    if accepted.len() != spec.accepted_draws as usize {
        return Err(SbiError::Acceptance(format!(
            "accepted {} of {} draws after {} proposals",
            accepted.len(),
            spec.accepted_draws,
            proposals_attempted
        )));
    }
    let rates = accepted
        .iter()
        .map(|row| row.growth_rate_per_time)
        .collect::<Vec<_>>();
    let mean = rates.iter().sum::<f64>() / rates.len() as f64;
    let sd = if rates.len() == 1 {
        0.0
    } else {
        (rates
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (rates.len() - 1) as f64)
            .sqrt()
    };
    Ok(RejectionAbcGrowthFrontResult {
        format: "marklab.rejection_abc_growth_front",
        version: 1,
        simulator: "marklab.growth_front_v1",
        summary: "final_total_density_mass",
        distance: "absolute_standardized_mass_residual",
        prior: "uniform_growth_rate",
        epsilon: spec.epsilon,
        accepted_draws_requested: spec.accepted_draws,
        proposals_attempted,
        acceptance_rate: f64::from(spec.accepted_draws) / f64::from(proposals_attempted),
        accepted,
        posterior: RejectionAbcPosterior {
            growth_rate_mean: mean,
            growth_rate_sd: sd,
            growth_rate_min: rates.iter().copied().fold(f64::INFINITY, f64::min),
            growth_rate_max: rates.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        },
        maximum_total_declared_cell_steps: maximum_total_work,
        random_seed_namespace: "rejection_abc_growth_front_v1_chacha20",
        claim_status: "synthetic_abc_specialization_not_biological_calibration",
    })
}

fn validate(spec: &RejectionAbcGrowthFrontSpec) -> Result<u64, SbiError> {
    let finite_positive = [
        spec.carrying_capacity,
        spec.final_time,
        spec.time_step,
        spec.mass_scale,
        spec.epsilon,
    ]
    .iter()
    .all(|value| value.is_finite() && *value > 0.0);
    let prior_valid = spec.growth_rate_prior_min.is_finite()
        && spec.growth_rate_prior_min >= 0.0
        && spec.growth_rate_prior_max.is_finite()
        && spec.growth_rate_prior_max > spec.growth_rate_prior_min;
    let controls_valid = spec.diffusion_um2_per_time.is_finite()
        && spec.diffusion_um2_per_time >= 0.0
        && spec.observed_final_mass.is_finite()
        && spec.observed_final_mass >= 0.0
        && spec.front_threshold_fraction.is_finite()
        && (0.0..1.0).contains(&spec.front_threshold_fraction)
        && (1..=10_000).contains(&spec.accepted_draws)
        && spec.maximum_proposals >= spec.accepted_draws
        && spec.maximum_proposals <= 1_000_000
        && (1..=250_000_000).contains(&spec.maximum_cell_steps_per_proposal);
    let maximum_total_work = u64::from(spec.maximum_proposals)
        .checked_mul(spec.maximum_cell_steps_per_proposal)
        .ok_or_else(|| SbiError::Invalid("ABC total work overflowed".into()))?;
    if !finite_positive || !prior_valid || !controls_valid || maximum_total_work > 250_000_000 {
        return Err(SbiError::Invalid(
            "ABC prior, summaries, controls, or total work are invalid".into(),
        ));
    }
    Ok(maximum_total_work)
}
