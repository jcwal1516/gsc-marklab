use marklab_simulation::{simulate_growth_front, GrowthFrontInitialPoint, GrowthFrontSpec};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;

use crate::SbiError;

#[derive(Clone, Debug)]
pub struct GrowthFrontPosteriorPredictiveLabSpec {
    pub initial: Vec<GrowthFrontInitialPoint>,
    pub posterior_growth_rate_draws: Vec<f64>,
    pub diffusion_um2_per_time: f64,
    pub carrying_capacity: f64,
    pub final_time: f64,
    pub time_step: f64,
    pub front_threshold_fraction: f64,
    pub observed_final_mass: f64,
    pub observed_maximum_density: f64,
    pub replicates: u32,
    pub interval_probability: f64,
    pub discrepancy_alpha: f64,
    pub maximum_cell_steps_per_replicate: u64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PosteriorPredictiveReplicate {
    pub replicate: u32,
    pub posterior_draw_index: u32,
    pub growth_rate_per_time: f64,
    pub final_mass: f64,
    pub maximum_density: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PosteriorPredictiveFailure {
    pub replicate: u32,
    pub posterior_draw_index: u32,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PosteriorPredictiveCheck {
    pub summary: &'static str,
    pub observed: f64,
    pub replicated_mean: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
    pub observed_in_interval: bool,
    pub lower_tail_probability: f64,
    pub upper_tail_probability: f64,
    pub two_sided_discrepancy_p_value: f64,
    pub misspecification_flag: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrowthFrontPosteriorPredictiveLabResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator: &'static str,
    pub summary_catalog: [&'static str; 2],
    pub interval_probability: f64,
    pub discrepancy_alpha: f64,
    pub replicates: Vec<PosteriorPredictiveReplicate>,
    pub failures: Vec<PosteriorPredictiveFailure>,
    pub failed_replicates: u32,
    pub checks: Vec<PosteriorPredictiveCheck>,
    pub misspecification_flags: Vec<&'static str>,
    pub maximum_total_declared_cell_steps: u64,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

pub fn growth_front_posterior_predictive_laboratory(
    spec: GrowthFrontPosteriorPredictiveLabSpec,
) -> Result<GrowthFrontPosteriorPredictiveLabResult, SbiError> {
    let maximum_work = validate(&spec)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut replicates = Vec::with_capacity(spec.replicates as usize);
    let mut failures = Vec::new();
    for replicate in 1..=spec.replicates {
        let draw_index = rng.gen_range(0..spec.posterior_growth_rate_draws.len());
        let growth_rate = spec.posterior_growth_rate_draws[draw_index];
        let result = simulate_growth_front(GrowthFrontSpec {
            initial: spec.initial.clone(),
            diffusion_um2_per_time: spec.diffusion_um2_per_time,
            growth_rate_per_time: growth_rate,
            carrying_capacity: spec.carrying_capacity,
            final_time: spec.final_time,
            time_step: spec.time_step,
            front_threshold_fraction: spec.front_threshold_fraction,
            record_every_steps: u32::MAX,
            maximum_cell_steps: spec.maximum_cell_steps_per_replicate,
        });
        match result {
            Ok(result) => replicates.push(PosteriorPredictiveReplicate {
                replicate,
                posterior_draw_index: draw_index as u32,
                growth_rate_per_time: growth_rate,
                final_mass: result.final_total_density_mass,
                maximum_density: result
                    .final_state
                    .iter()
                    .map(|row| row.density)
                    .fold(f64::NEG_INFINITY, f64::max),
            }),
            Err(error) => failures.push(PosteriorPredictiveFailure {
                replicate,
                posterior_draw_index: draw_index as u32,
                reason: error.to_string(),
            }),
        }
    }
    if replicates.is_empty() {
        return Err(SbiError::Acceptance(
            "all posterior-predictive replicates failed".into(),
        ));
    }
    let checks = vec![
        check(
            "final_total_density_mass",
            spec.observed_final_mass,
            replicates.iter().map(|row| row.final_mass).collect(),
            spec.interval_probability,
            spec.discrepancy_alpha,
        ),
        check(
            "maximum_final_density",
            spec.observed_maximum_density,
            replicates.iter().map(|row| row.maximum_density).collect(),
            spec.interval_probability,
            spec.discrepancy_alpha,
        ),
    ];
    let misspecification_flags = checks
        .iter()
        .filter(|row| row.misspecification_flag)
        .map(|row| row.summary)
        .collect();
    Ok(GrowthFrontPosteriorPredictiveLabResult {
        format: "marklab.growth_front_posterior_predictive_laboratory",
        version: 1,
        simulator: "marklab.growth_front_v1",
        summary_catalog: ["final_total_density_mass", "maximum_final_density"],
        interval_probability: spec.interval_probability,
        discrepancy_alpha: spec.discrepancy_alpha,
        failed_replicates: failures.len() as u32,
        replicates,
        failures,
        checks,
        misspecification_flags,
        maximum_total_declared_cell_steps: maximum_work,
        random_seed_namespace: "growth_front_ppc_lab_v1_chacha20",
        claim_status: "synthetic_posterior_predictive_check_not_model_validation",
    })
}

fn check(
    summary: &'static str,
    observed: f64,
    mut values: Vec<f64>,
    interval_probability: f64,
    alpha: f64,
) -> PosteriorPredictiveCheck {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let lower_tail = (1 + values.iter().filter(|value| **value <= observed).count()) as f64
        / (values.len() + 1) as f64;
    let upper_tail = (1 + values.iter().filter(|value| **value >= observed).count()) as f64
        / (values.len() + 1) as f64;
    let two_sided = (2.0 * lower_tail.min(upper_tail)).min(1.0);
    values.sort_by(f64::total_cmp);
    let tail = (1.0 - interval_probability) / 2.0;
    let lower_index = (tail * values.len() as f64).floor() as usize;
    let upper_index = ((1.0 - tail) * values.len() as f64).ceil() as usize - 1;
    let lower = values[lower_index.min(values.len() - 1)];
    let upper = values[upper_index.min(values.len() - 1)];
    let in_interval = (lower..=upper).contains(&observed);
    PosteriorPredictiveCheck {
        summary,
        observed,
        replicated_mean: mean,
        interval_lower: lower,
        interval_upper: upper,
        observed_in_interval: in_interval,
        lower_tail_probability: lower_tail,
        upper_tail_probability: upper_tail,
        two_sided_discrepancy_p_value: two_sided,
        misspecification_flag: !in_interval || two_sided < alpha,
    }
}

fn validate(spec: &GrowthFrontPosteriorPredictiveLabSpec) -> Result<u64, SbiError> {
    let numeric = [spec.carrying_capacity, spec.final_time, spec.time_step]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
        && spec.diffusion_um2_per_time.is_finite()
        && spec.diffusion_um2_per_time >= 0.0
        && spec.front_threshold_fraction.is_finite()
        && (0.0..1.0).contains(&spec.front_threshold_fraction)
        && spec.observed_final_mass.is_finite()
        && spec.observed_maximum_density.is_finite()
        && (2..=1_000_000).contains(&spec.posterior_growth_rate_draws.len())
        && spec
            .posterior_growth_rate_draws
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        && (20..=100_000).contains(&spec.replicates)
        && spec.interval_probability.is_finite()
        && (0.0..1.0).contains(&spec.interval_probability)
        && spec.discrepancy_alpha.is_finite()
        && (0.0..1.0).contains(&spec.discrepancy_alpha)
        && (1..=250_000_000).contains(&spec.maximum_cell_steps_per_replicate);
    let work = u64::from(spec.replicates)
        .checked_mul(spec.maximum_cell_steps_per_replicate)
        .ok_or_else(|| SbiError::Invalid("posterior-predictive work overflowed".into()))?;
    if !numeric || work > 250_000_000 {
        return Err(SbiError::Invalid(
            "posterior-predictive inputs or work are invalid".into(),
        ));
    }
    Ok(work)
}
