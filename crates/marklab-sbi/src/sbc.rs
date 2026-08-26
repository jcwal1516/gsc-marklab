use marklab_simulation::{simulate_growth_front, GrowthFrontInitialPoint, GrowthFrontSpec};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;

use crate::{rejection_abc_growth_front, RejectionAbcGrowthFrontSpec, SbiError};

#[derive(Clone, Debug)]
pub struct GrowthFrontRejectionAbcSbcSpec {
    pub initial: Vec<GrowthFrontInitialPoint>,
    pub diffusion_um2_per_time: f64,
    pub carrying_capacity: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub front_threshold_fraction: f64,
    pub mass_scale: f64,
    pub growth_rate_prior_min: f64,
    pub growth_rate_prior_max: f64,
    pub epsilon: f64,
    pub posterior_draws: u32,
    pub maximum_proposals_per_replicate: u32,
    pub replicates: u32,
    pub coverage_probability: f64,
    pub maximum_cell_steps_per_simulation: u64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SbcReplicate {
    pub replicate: u32,
    pub true_growth_rate_per_time: f64,
    pub observed_final_mass: f64,
    pub rank: Option<u32>,
    pub normalized_rank: Option<f64>,
    pub interval_lower: Option<f64>,
    pub interval_upper: Option<f64>,
    pub covered: Option<bool>,
    pub proposals_attempted: Option<u32>,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct SbcRankDiagnostics {
    pub possible_ranks: u32,
    pub histogram: Vec<u32>,
    pub mean_normalized_rank: f64,
    pub maximum_ecdf_deviation: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SbcCoverage {
    pub nominal: f64,
    pub empirical: f64,
    pub covered: u32,
    pub assessed: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrowthFrontRejectionAbcSbcResult {
    pub format: &'static str,
    pub version: u32,
    pub prior: &'static str,
    pub simulator: &'static str,
    pub inference_algorithm: &'static str,
    pub replicates: Vec<SbcReplicate>,
    pub failed_replicates: u32,
    pub rank_diagnostics: SbcRankDiagnostics,
    pub coverage: SbcCoverage,
    pub maximum_total_declared_cell_steps: u64,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

pub fn growth_front_rejection_abc_sbc(
    spec: GrowthFrontRejectionAbcSbcSpec,
) -> Result<GrowthFrontRejectionAbcSbcResult, SbiError> {
    let maximum_work = validate(&spec)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut rows = Vec::with_capacity(spec.replicates as usize);
    let mut histogram = vec![0_u32; spec.posterior_draws as usize + 1];
    let mut successful_ranks = Vec::new();
    let mut covered_count = 0_u32;
    let mut failed = 0_u32;

    for replicate in 1..=spec.replicates {
        let truth = rng.gen_range(spec.growth_rate_prior_min..spec.growth_rate_prior_max);
        let observed = simulate_growth_front(GrowthFrontSpec {
            initial: spec.initial.clone(),
            diffusion_um2_per_time: spec.diffusion_um2_per_time,
            growth_rate_per_time: truth,
            carrying_capacity: spec.carrying_capacity,
            final_time: spec.final_time,
            time_step: spec.time_step,
            front_threshold_fraction: spec.front_threshold_fraction,
            record_every_steps: u32::MAX,
            maximum_cell_steps: spec.maximum_cell_steps_per_simulation,
        })?;
        let inference = rejection_abc_growth_front(RejectionAbcGrowthFrontSpec {
            initial: spec.initial.clone(),
            diffusion_um2_per_time: spec.diffusion_um2_per_time,
            carrying_capacity: spec.carrying_capacity,
            final_time: spec.final_time,
            time_step: spec.time_step,
            front_threshold_fraction: spec.front_threshold_fraction,
            observed_final_mass: observed.final_total_density_mass,
            mass_scale: spec.mass_scale,
            growth_rate_prior_min: spec.growth_rate_prior_min,
            growth_rate_prior_max: spec.growth_rate_prior_max,
            epsilon: spec.epsilon,
            accepted_draws: spec.posterior_draws,
            maximum_proposals: spec.maximum_proposals_per_replicate,
            maximum_cell_steps_per_proposal: spec.maximum_cell_steps_per_simulation,
            seed: replicate_seed(spec.seed, replicate),
        });
        let inference = match inference {
            Ok(inference) => inference,
            Err(SbiError::Acceptance(_)) => {
                failed += 1;
                rows.push(SbcReplicate {
                    replicate,
                    true_growth_rate_per_time: truth,
                    observed_final_mass: observed.final_total_density_mass,
                    rank: None,
                    normalized_rank: None,
                    interval_lower: None,
                    interval_upper: None,
                    covered: None,
                    proposals_attempted: None,
                    status: "inference_failed",
                });
                continue;
            }
            Err(error) => return Err(error),
        };
        let mut draws = inference
            .accepted
            .iter()
            .map(|row| row.growth_rate_per_time)
            .collect::<Vec<_>>();
        let rank = draws.iter().filter(|value| **value < truth).count() as u32;
        histogram[rank as usize] += 1;
        let normalized_rank = f64::from(rank) / f64::from(spec.posterior_draws);
        successful_ranks.push(normalized_rank);
        draws.sort_by(f64::total_cmp);
        let alpha = (1.0 - spec.coverage_probability) / 2.0;
        let lower_index = (alpha * draws.len() as f64).floor() as usize;
        let upper_index = ((1.0 - alpha) * draws.len() as f64).ceil() as usize - 1;
        let lower = draws[lower_index.min(draws.len() - 1)];
        let upper = draws[upper_index.min(draws.len() - 1)];
        let covered = (lower..=upper).contains(&truth);
        covered_count += u32::from(covered);
        rows.push(SbcReplicate {
            replicate,
            true_growth_rate_per_time: truth,
            observed_final_mass: observed.final_total_density_mass,
            rank: Some(rank),
            normalized_rank: Some(normalized_rank),
            interval_lower: Some(lower),
            interval_upper: Some(upper),
            covered: Some(covered),
            proposals_attempted: Some(inference.proposals_attempted),
            status: "complete",
        });
    }
    if successful_ranks.is_empty() {
        return Err(SbiError::Acceptance(
            "all SBC inference replicates failed".into(),
        ));
    }
    successful_ranks.sort_by(f64::total_cmp);
    let mean_rank = successful_ranks.iter().sum::<f64>() / successful_ranks.len() as f64;
    let max_deviation = successful_ranks
        .iter()
        .enumerate()
        .map(|(index, rank)| ((index + 1) as f64 / successful_ranks.len() as f64 - rank).abs())
        .fold(0.0, f64::max);
    let assessed = successful_ranks.len() as u32;
    Ok(GrowthFrontRejectionAbcSbcResult {
        format: "marklab.growth_front_rejection_abc_sbc",
        version: 1,
        prior: "uniform_growth_rate",
        simulator: "marklab.growth_front_v1",
        inference_algorithm: "rejection_abc_growth_front_v1",
        replicates: rows,
        failed_replicates: failed,
        rank_diagnostics: SbcRankDiagnostics {
            possible_ranks: spec.posterior_draws + 1,
            histogram,
            mean_normalized_rank: mean_rank,
            maximum_ecdf_deviation: max_deviation,
        },
        coverage: SbcCoverage {
            nominal: spec.coverage_probability,
            empirical: f64::from(covered_count) / f64::from(assessed),
            covered: covered_count,
            assessed,
        },
        maximum_total_declared_cell_steps: maximum_work,
        random_seed_namespace: "growth_front_rejection_abc_sbc_v1_chacha20",
        claim_status: "synthetic_sbc_diagnostic_not_biological_calibration",
    })
}

fn replicate_seed(seed: u64, replicate: u32) -> u64 {
    seed ^ u64::from(replicate).wrapping_mul(0xd1b5_4a32_d192_ed03)
}

fn validate(spec: &GrowthFrontRejectionAbcSbcSpec) -> Result<u64, SbiError> {
    let numeric = [
        spec.carrying_capacity,
        spec.final_time,
        spec.time_step,
        spec.mass_scale,
        spec.epsilon,
    ]
    .iter()
    .all(|value| value.is_finite() && *value > 0.0)
        && spec.diffusion_um2_per_time.is_finite()
        && spec.diffusion_um2_per_time >= 0.0
        && spec.growth_rate_prior_min.is_finite()
        && spec.growth_rate_prior_min >= 0.0
        && spec.growth_rate_prior_max.is_finite()
        && spec.growth_rate_prior_max > spec.growth_rate_prior_min
        && spec.front_threshold_fraction.is_finite()
        && (0.0..1.0).contains(&spec.front_threshold_fraction)
        && (1..=10_000).contains(&spec.posterior_draws)
        && spec.maximum_proposals_per_replicate >= spec.posterior_draws
        && spec.maximum_proposals_per_replicate <= 1_000_000
        && (20..=10_000).contains(&spec.replicates)
        && spec.coverage_probability.is_finite()
        && (0.0..1.0).contains(&spec.coverage_probability)
        && (1..=250_000_000).contains(&spec.maximum_cell_steps_per_simulation);
    let work = u64::from(spec.replicates)
        .checked_mul(u64::from(spec.maximum_proposals_per_replicate) + 1)
        .and_then(|value| value.checked_mul(spec.maximum_cell_steps_per_simulation))
        .ok_or_else(|| SbiError::Invalid("SBC work overflowed".into()))?;
    if !numeric || work > 250_000_000 {
        return Err(SbiError::Invalid(
            "SBC controls or aggregate work are invalid".into(),
        ));
    }
    Ok(work)
}
