use marklab_simulation::{simulate_growth_front, GrowthFrontInitialPoint, GrowthFrontSpec};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::StandardNormal;
use serde::{Deserialize, Deserializer, Serialize};

use crate::SbiError;

#[derive(Clone, Debug)]
pub struct SmcAbcGrowthFrontSpec {
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
    pub epsilon_schedule: Vec<f64>,
    pub particles: u32,
    pub maximum_proposals_per_stage: u32,
    pub maximum_cell_steps_per_proposal: u64,
    pub seed: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmcAbcParticle {
    pub growth_rate_per_time: f64,
    pub distance: f64,
    pub weight: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmcAbcStage {
    pub stage: u32,
    pub epsilon: f64,
    pub proposals_attempted: u32,
    pub acceptance_rate: f64,
    pub next_perturbation_sd: f64,
    pub effective_sample_size: f64,
    pub weighted_growth_rate_mean: f64,
    pub weighted_growth_rate_sd: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmcAbcPosterior {
    pub growth_rate_mean: f64,
    pub growth_rate_sd: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SmcAbcGrowthFrontResult {
    pub format: &'static str,
    pub version: u32,
    pub simulator: &'static str,
    pub summary: &'static str,
    pub prior: &'static str,
    pub stages: Vec<SmcAbcStage>,
    pub particles: Vec<SmcAbcParticle>,
    pub posterior: SmcAbcPosterior,
    pub total_simulations: u64,
    pub maximum_total_declared_cell_steps: u64,
    pub random_seed_namespace: &'static str,
    pub claim_status: &'static str,
}

impl<'de> Deserialize<'de> for SmcAbcGrowthFrontResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct OwnedResult {
            format: String,
            version: u32,
            simulator: String,
            summary: String,
            prior: String,
            stages: Vec<SmcAbcStage>,
            particles: Vec<SmcAbcParticle>,
            posterior: SmcAbcPosterior,
            total_simulations: u64,
            maximum_total_declared_cell_steps: u64,
            random_seed_namespace: String,
            claim_status: String,
        }

        let owned = OwnedResult::deserialize(deserializer)?;
        let expected = [
            (&owned.format, "marklab.smc_abc_growth_front", "format"),
            (&owned.simulator, "marklab.growth_front_v1", "simulator"),
            (&owned.summary, "final_total_density_mass", "summary"),
            (&owned.prior, "uniform_growth_rate", "prior"),
            (
                &owned.random_seed_namespace,
                "smc_abc_growth_front_v1_chacha20",
                "random_seed_namespace",
            ),
            (
                &owned.claim_status,
                "synthetic_smc_abc_specialization_not_biological_calibration",
                "claim_status",
            ),
        ];
        for (observed, value, field) in expected {
            if observed != value {
                return Err(serde::de::Error::custom(format!(
                    "unexpected SMC-ABC {field}: {observed}"
                )));
            }
        }
        Ok(Self {
            format: "marklab.smc_abc_growth_front",
            version: owned.version,
            simulator: "marklab.growth_front_v1",
            summary: "final_total_density_mass",
            prior: "uniform_growth_rate",
            stages: owned.stages,
            particles: owned.particles,
            posterior: owned.posterior,
            total_simulations: owned.total_simulations,
            maximum_total_declared_cell_steps: owned.maximum_total_declared_cell_steps,
            random_seed_namespace: "smc_abc_growth_front_v1_chacha20",
            claim_status: "synthetic_smc_abc_specialization_not_biological_calibration",
        })
    }
}

pub fn smc_abc_growth_front(
    spec: SmcAbcGrowthFrontSpec,
) -> Result<SmcAbcGrowthFrontResult, SbiError> {
    let maximum_work = validate(&spec)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut previous = Vec::<SmcAbcParticle>::new();
    let mut stages = Vec::with_capacity(spec.epsilon_schedule.len());
    let mut total_simulations = 0_u64;
    let range = spec.growth_rate_prior_max - spec.growth_rate_prior_min;
    let mut perturbation_sd = range / 4.0;

    for (stage_index, epsilon) in spec.epsilon_schedule.iter().copied().enumerate() {
        let mut particles = Vec::with_capacity(spec.particles as usize);
        let mut attempts = 0_u32;
        while particles.len() < spec.particles as usize
            && attempts < spec.maximum_proposals_per_stage
        {
            attempts += 1;
            let theta = if stage_index == 0 {
                rng.gen_range(spec.growth_rate_prior_min..spec.growth_rate_prior_max)
            } else {
                let ancestor = sample_ancestor(&previous, &mut rng);
                ancestor + rng.sample::<f64, _>(StandardNormal) * perturbation_sd
            };
            if !(spec.growth_rate_prior_min..spec.growth_rate_prior_max).contains(&theta) {
                continue;
            }
            total_simulations += 1;
            let distance = simulate_distance(&spec, theta)?;
            if distance > epsilon {
                continue;
            }
            let weight = if stage_index == 0 {
                1.0
            } else {
                let denominator = previous
                    .iter()
                    .map(|row| {
                        row.weight
                            * normal_density(theta, row.growth_rate_per_time, perturbation_sd)
                    })
                    .sum::<f64>();
                range.recip() / denominator
            };
            if !weight.is_finite() || weight <= 0.0 {
                return Err(SbiError::Acceptance(
                    "SMC-ABC importance weight became invalid".into(),
                ));
            }
            particles.push(SmcAbcParticle {
                growth_rate_per_time: theta,
                distance,
                weight,
            });
        }
        if particles.len() != spec.particles as usize {
            return Err(SbiError::Acceptance(format!(
                "stage {} accepted {} of {} particles after {} proposals",
                stage_index + 1,
                particles.len(),
                spec.particles,
                attempts
            )));
        }
        normalize(&mut particles)?;
        let (mean, sd) = weighted_moments(&particles);
        let ess = particles
            .iter()
            .map(|row| row.weight.powi(2))
            .sum::<f64>()
            .recip();
        perturbation_sd = (2.0_f64).sqrt() * sd.max(range * 1e-6);
        stages.push(SmcAbcStage {
            stage: stage_index as u32 + 1,
            epsilon,
            proposals_attempted: attempts,
            acceptance_rate: f64::from(spec.particles) / f64::from(attempts),
            next_perturbation_sd: perturbation_sd,
            effective_sample_size: ess,
            weighted_growth_rate_mean: mean,
            weighted_growth_rate_sd: sd,
        });
        previous = particles;
    }
    let (mean, sd) = weighted_moments(&previous);
    Ok(SmcAbcGrowthFrontResult {
        format: "marklab.smc_abc_growth_front",
        version: 1,
        simulator: "marklab.growth_front_v1",
        summary: "final_total_density_mass",
        prior: "uniform_growth_rate",
        stages,
        particles: previous,
        posterior: SmcAbcPosterior {
            growth_rate_mean: mean,
            growth_rate_sd: sd,
        },
        total_simulations,
        maximum_total_declared_cell_steps: maximum_work,
        random_seed_namespace: "smc_abc_growth_front_v1_chacha20",
        claim_status: "synthetic_smc_abc_specialization_not_biological_calibration",
    })
}

fn simulate_distance(spec: &SmcAbcGrowthFrontSpec, growth_rate: f64) -> Result<f64, SbiError> {
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
    Ok(((simulation.final_total_density_mass - spec.observed_final_mass) / spec.mass_scale).abs())
}

fn sample_ancestor(particles: &[SmcAbcParticle], rng: &mut ChaCha20Rng) -> f64 {
    let selection = rng.gen::<f64>();
    let mut cumulative = 0.0;
    for particle in particles {
        cumulative += particle.weight;
        if selection < cumulative {
            return particle.growth_rate_per_time;
        }
    }
    particles
        .last()
        .expect("nonempty prior stage")
        .growth_rate_per_time
}

fn normal_density(value: f64, mean: f64, sd: f64) -> f64 {
    let standardized = (value - mean) / sd;
    (-0.5 * standardized.powi(2)).exp() / (sd * (2.0 * std::f64::consts::PI).sqrt())
}

fn normalize(particles: &mut [SmcAbcParticle]) -> Result<(), SbiError> {
    let total = particles.iter().map(|row| row.weight).sum::<f64>();
    if !total.is_finite() || total <= 0.0 {
        return Err(SbiError::Acceptance(
            "SMC-ABC weight normalization failed".into(),
        ));
    }
    for particle in particles {
        particle.weight /= total;
    }
    Ok(())
}

fn weighted_moments(particles: &[SmcAbcParticle]) -> (f64, f64) {
    let mean = particles
        .iter()
        .map(|row| row.weight * row.growth_rate_per_time)
        .sum::<f64>();
    let variance = particles
        .iter()
        .map(|row| row.weight * (row.growth_rate_per_time - mean).powi(2))
        .sum::<f64>();
    (mean, variance.max(0.0).sqrt())
}

fn validate(spec: &SmcAbcGrowthFrontSpec) -> Result<u64, SbiError> {
    let epsilon_valid = (1..=32).contains(&spec.epsilon_schedule.len())
        && spec
            .epsilon_schedule
            .iter()
            .enumerate()
            .all(|(index, value)| {
                value.is_finite()
                    && *value > 0.0
                    && (index == 0 || spec.epsilon_schedule[index - 1] > *value)
            });
    let numeric_valid = [
        spec.carrying_capacity,
        spec.final_time,
        spec.time_step,
        spec.mass_scale,
    ]
    .iter()
    .all(|value| value.is_finite() && *value > 0.0)
        && spec.diffusion_um2_per_time.is_finite()
        && spec.diffusion_um2_per_time >= 0.0
        && spec.observed_final_mass.is_finite()
        && spec.observed_final_mass >= 0.0
        && spec.growth_rate_prior_min.is_finite()
        && spec.growth_rate_prior_min >= 0.0
        && spec.growth_rate_prior_max.is_finite()
        && spec.growth_rate_prior_max > spec.growth_rate_prior_min
        && spec.front_threshold_fraction.is_finite()
        && (0.0..1.0).contains(&spec.front_threshold_fraction)
        && (2..=10_000).contains(&spec.particles)
        && spec.maximum_proposals_per_stage >= spec.particles
        && spec.maximum_proposals_per_stage <= 1_000_000;
    let work = (spec.epsilon_schedule.len() as u64)
        .checked_mul(u64::from(spec.maximum_proposals_per_stage))
        .and_then(|value| value.checked_mul(spec.maximum_cell_steps_per_proposal))
        .ok_or_else(|| SbiError::Invalid("SMC-ABC total work overflowed".into()))?;
    if !epsilon_valid || !numeric_valid || work > 250_000_000 {
        return Err(SbiError::Invalid(
            "SMC-ABC schedule, prior, controls, or total work are invalid".into(),
        ));
    }
    Ok(work)
}
