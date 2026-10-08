use marklab_simulation::{simulate_growth_front, GrowthFrontInitialPoint, GrowthFrontSpec};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::StandardNormal;
use serde::Serialize;

use crate::SbiError;

#[derive(Clone, Debug)]
pub struct SyntheticLikelihoodGrowthFrontSpec {
    pub initial: Vec<GrowthFrontInitialPoint>,
    pub diffusion_um2_per_time: f64,
    pub carrying_capacity: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub front_threshold_fraction: f64,
    pub observed_final_mass: f64,
    pub observed_maximum_density: f64,
    pub mass_noise_sd: f64,
    pub maximum_density_noise_sd: f64,
    pub growth_rate_prior_min: f64,
    pub growth_rate_prior_max: f64,
    pub replicates: u32,
    pub covariance_shrinkage: f64,
    pub iterations: u32,
    pub burn_in: u32,
    pub proposal_sd: f64,
    pub maximum_cell_steps_per_simulation: u64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyntheticLikelihoodEstimate {
    pub summary_dimension: u32,
    pub replicates_per_evaluation: u32,
    pub mean_final_mass: f64,
    pub mean_maximum_density: f64,
    pub covariance: [[f64; 2]; 2],
    pub determinant: f64,
    pub log_likelihood: f64,
    pub mean_monte_carlo_se: [f64; 2],
    pub covariance_shrinkage: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyntheticLikelihoodTraceRow {
    pub iteration: u32,
    pub growth_rate_per_time: f64,
    pub log_synthetic_likelihood: f64,
    pub accepted: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyntheticLikelihoodMcmcDiagnostics {
    pub iterations: u32,
    pub burn_in: u32,
    pub retained_draws: u32,
    pub accepted_proposals: u32,
    pub acceptance_rate: f64,
    pub proposal: &'static str,
    pub noisy_likelihood_state: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyntheticLikelihoodPosterior {
    pub growth_rate_mean: f64,
    pub growth_rate_sd: f64,
    pub growth_rate_min: f64,
    pub growth_rate_max: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyntheticLikelihoodGrowthFrontResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator: &'static str,
    pub summaries: [&'static str; 2],
    pub likelihood: SyntheticLikelihoodEstimate,
    pub mcmc: SyntheticLikelihoodMcmcDiagnostics,
    pub posterior: SyntheticLikelihoodPosterior,
    pub trace: Vec<SyntheticLikelihoodTraceRow>,
    pub maximum_total_declared_cell_steps: u64,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

pub fn synthetic_likelihood_growth_front(
    spec: SyntheticLikelihoodGrowthFrontSpec,
) -> Result<SyntheticLikelihoodGrowthFrontResult, SbiError> {
    let maximum_work = validate(&spec)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut current = (spec.growth_rate_prior_min + spec.growth_rate_prior_max) / 2.0;
    let mut current_likelihood = estimate(&spec, current, &mut rng)?;
    let mut accepted_proposals = 0_u32;
    let mut trace = Vec::with_capacity(spec.iterations as usize);
    for iteration in 1..=spec.iterations {
        let proposal = current + rng.sample::<f64, _>(StandardNormal) * spec.proposal_sd;
        let mut accepted = false;
        if (spec.growth_rate_prior_min..spec.growth_rate_prior_max).contains(&proposal) {
            let proposal_likelihood = estimate(&spec, proposal, &mut rng)?;
            let log_acceptance =
                proposal_likelihood.log_likelihood - current_likelihood.log_likelihood;
            if rng.gen::<f64>().ln() < log_acceptance.min(0.0) {
                current = proposal;
                current_likelihood = proposal_likelihood;
                accepted = true;
                accepted_proposals += 1;
            }
        }
        trace.push(SyntheticLikelihoodTraceRow {
            iteration,
            growth_rate_per_time: current,
            log_synthetic_likelihood: current_likelihood.log_likelihood,
            accepted,
        });
    }
    let retained = trace
        .iter()
        .skip(spec.burn_in as usize)
        .map(|row| row.growth_rate_per_time)
        .collect::<Vec<_>>();
    let mean = retained.iter().sum::<f64>() / retained.len() as f64;
    let sd = if retained.len() == 1 {
        0.0
    } else {
        (retained
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (retained.len() - 1) as f64)
            .sqrt()
    };
    Ok(SyntheticLikelihoodGrowthFrontResult {
        format: "marklab.synthetic_likelihood_growth_front",
        version: 1,
        simulator: "marklab.growth_front_v1_with_declared_gaussian_summary_noise",
        summaries: ["final_total_density_mass", "maximum_final_density"],
        likelihood: current_likelihood,
        mcmc: SyntheticLikelihoodMcmcDiagnostics {
            iterations: spec.iterations,
            burn_in: spec.burn_in,
            retained_draws: retained.len() as u32,
            accepted_proposals,
            acceptance_rate: f64::from(accepted_proposals) / f64::from(spec.iterations),
            proposal: "gaussian_random_walk",
            noisy_likelihood_state: "retain_current_estimate_on_rejection",
        },
        posterior: SyntheticLikelihoodPosterior {
            growth_rate_mean: mean,
            growth_rate_sd: sd,
            growth_rate_min: retained.iter().copied().fold(f64::INFINITY, f64::min),
            growth_rate_max: retained.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        },
        trace,
        maximum_total_declared_cell_steps: maximum_work,
        random_seed_namespace: "synthetic_likelihood_growth_front_v1_chacha20",
        claim_status: "approximate_synthetic_likelihood_not_biological_calibration",
    })
}

fn estimate(
    spec: &SyntheticLikelihoodGrowthFrontSpec,
    growth_rate: f64,
    rng: &mut ChaCha20Rng,
) -> Result<SyntheticLikelihoodEstimate, SbiError> {
    let mut summaries = Vec::with_capacity(spec.replicates as usize);
    for _ in 0..spec.replicates {
        let simulation = simulate_growth_front(GrowthFrontSpec {
            initial: spec.initial.clone(),
            diffusion_um2_per_time: spec.diffusion_um2_per_time,
            growth_rate_per_time: growth_rate,
            carrying_capacity: spec.carrying_capacity,
            final_time: spec.final_time,
            time_step: spec.time_step,
            front_threshold_fraction: spec.front_threshold_fraction,
            record_every_steps: u32::MAX,
            maximum_cell_steps: spec.maximum_cell_steps_per_simulation,
        })?;
        let maximum_density = simulation
            .final_state
            .iter()
            .map(|row| row.density)
            .fold(f64::NEG_INFINITY, f64::max);
        summaries.push([
            simulation.final_total_density_mass
                + rng.sample::<f64, _>(StandardNormal) * spec.mass_noise_sd,
            maximum_density + rng.sample::<f64, _>(StandardNormal) * spec.maximum_density_noise_sd,
        ]);
    }
    let mean = [
        summaries.iter().map(|row| row[0]).sum::<f64>() / summaries.len() as f64,
        summaries.iter().map(|row| row[1]).sum::<f64>() / summaries.len() as f64,
    ];
    let mut covariance = [[0.0; 2]; 2];
    for row in &summaries {
        for i in 0..2 {
            for j in 0..2 {
                covariance[i][j] += (row[i] - mean[i]) * (row[j] - mean[j]);
            }
        }
    }
    let denominator = f64::from(spec.replicates - 1);
    for row in &mut covariance {
        for value in row {
            *value /= denominator;
        }
    }
    covariance[0][1] *= 1.0 - spec.covariance_shrinkage;
    covariance[1][0] = covariance[0][1];
    let determinant = covariance[0][0] * covariance[1][1] - covariance[0][1].powi(2);
    if !determinant.is_finite() || determinant <= 0.0 {
        return Err(SbiError::Acceptance(
            "synthetic summary covariance is not positive definite".into(),
        ));
    }
    let residual = [
        spec.observed_final_mass - mean[0],
        spec.observed_maximum_density - mean[1],
    ];
    let quadratic = (covariance[1][1] * residual[0].powi(2)
        - 2.0 * covariance[0][1] * residual[0] * residual[1]
        + covariance[0][0] * residual[1].powi(2))
        / determinant;
    let log_likelihood =
        -0.5 * (2.0 * (2.0 * std::f64::consts::PI).ln() + determinant.ln() + quadratic);
    Ok(SyntheticLikelihoodEstimate {
        summary_dimension: 2,
        replicates_per_evaluation: spec.replicates,
        mean_final_mass: mean[0],
        mean_maximum_density: mean[1],
        covariance,
        determinant,
        log_likelihood,
        mean_monte_carlo_se: [
            (covariance[0][0] / f64::from(spec.replicates)).sqrt(),
            (covariance[1][1] / f64::from(spec.replicates)).sqrt(),
        ],
        covariance_shrinkage: spec.covariance_shrinkage,
    })
}

fn validate(spec: &SyntheticLikelihoodGrowthFrontSpec) -> Result<u64, SbiError> {
    let positive = [
        spec.carrying_capacity,
        spec.final_time,
        spec.time_step,
        spec.mass_noise_sd,
        spec.maximum_density_noise_sd,
        spec.proposal_sd,
    ]
    .iter()
    .all(|value| value.is_finite() && *value > 0.0);
    let controls = spec.diffusion_um2_per_time.is_finite()
        && spec.diffusion_um2_per_time >= 0.0
        && spec.observed_final_mass.is_finite()
        && spec.observed_maximum_density.is_finite()
        && spec.growth_rate_prior_min.is_finite()
        && spec.growth_rate_prior_min >= 0.0
        && spec.growth_rate_prior_max.is_finite()
        && spec.growth_rate_prior_max > spec.growth_rate_prior_min
        && spec.front_threshold_fraction.is_finite()
        && (0.0..1.0).contains(&spec.front_threshold_fraction)
        && (8..=10_000).contains(&spec.replicates)
        && spec.covariance_shrinkage.is_finite()
        && (0.0..=1.0).contains(&spec.covariance_shrinkage)
        && (10..=100_000).contains(&spec.iterations)
        && spec.burn_in < spec.iterations
        && (1..=250_000_000).contains(&spec.maximum_cell_steps_per_simulation);
    let work = u64::from(spec.iterations + 1)
        .checked_mul(u64::from(spec.replicates))
        .and_then(|value| value.checked_mul(spec.maximum_cell_steps_per_simulation))
        .ok_or_else(|| SbiError::Invalid("synthetic-likelihood work overflowed".into()))?;
    if !positive || !controls || work > 250_000_000 {
        return Err(SbiError::Invalid(
            "synthetic-likelihood controls or total work are invalid".into(),
        ));
    }
    Ok(work)
}
