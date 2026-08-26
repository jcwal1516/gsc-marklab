use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use thiserror::Error;

use crate::{
    strauss::{neighbor_delta, papangelou_from_delta},
    thomas::derive_seed,
    RectangularWindow, StraussPoint,
};

#[derive(Clone, Debug)]
pub struct StraussBirthDeathSpec {
    pub window: RectangularWindow,
    pub beta_per_um2: f64,
    pub gamma: f64,
    pub interaction_radius_um: f64,
    pub iterations: u32,
    pub burn_in: u32,
    pub seed: u64,
    pub maximum_points: u32,
    pub maximum_neighbor_visits: u64,
}

#[derive(Debug, Error)]
pub enum StraussBirthDeathError {
    #[error("invalid Strauss birth/death specification: {0}")]
    InvalidSpec(String),
    #[error("Strauss birth/death resource limit exceeded: {0}")]
    Resource(String),
    #[error("Strauss birth/death numerical failure: {0}")]
    Numerical(String),
}

#[derive(Debug, Serialize)]
pub struct StraussBirthDeathDiagnostics {
    pub birth_proposals: u64,
    pub accepted_births: u64,
    pub death_proposals: u64,
    pub accepted_deaths: u64,
    pub empty_death_transitions: u64,
    pub rejected_births: u64,
    pub rejected_deaths: u64,
    pub neighbor_visits: u64,
    pub maximum_observed_count: u32,
    pub final_count: u32,
    pub post_burn_state_count: u32,
    pub post_burn_mean_count: f64,
    pub post_burn_first_half_mean_count: f64,
    pub post_burn_second_half_mean_count: f64,
    pub absolute_half_mean_drift: f64,
    pub poisson_special_case_expected_count: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct StraussBirthDeathResult {
    pub format: &'static str,
    pub version: u32,
    pub coordinate_unit: &'static str,
    pub window: RectangularWindow,
    pub beta_per_um2: f64,
    pub gamma: f64,
    pub interaction_radius_um: f64,
    pub pair_boundary: &'static str,
    pub iterations: u32,
    pub burn_in: u32,
    pub seed: u64,
    pub rng: &'static str,
    pub initial_state: &'static str,
    pub transition_rule: &'static str,
    pub maximum_points: u32,
    pub maximum_neighbor_visits: u64,
    pub diagnostics: StraussBirthDeathDiagnostics,
    pub count_trace: Vec<u32>,
    pub final_pattern: Vec<StraussPoint>,
    pub claim_status: &'static str,
}

pub fn simulate_strauss_birth_death(
    spec: StraussBirthDeathSpec,
) -> Result<StraussBirthDeathResult, StraussBirthDeathError> {
    validate(&spec)?;
    let width = spec.window.xmax_um - spec.window.xmin_um;
    let height = spec.window.ymax_um - spec.window.ymin_um;
    let area = width * height;
    if !area.is_finite() || area <= 0.0 || !(spec.beta_per_um2 * area).is_finite() {
        return Err(StraussBirthDeathError::Numerical(
            "Strauss window area or beta-area product is non-finite".into(),
        ));
    }
    let mut rng =
        ChaCha20Rng::from_seed(derive_seed(b"marklab-strauss-birth-death-v1\0", spec.seed));
    let mut points = Vec::<(f64, f64)>::new();
    let mut trace = Vec::with_capacity(spec.iterations as usize + 1);
    trace.push(0);
    let mut birth_proposals = 0_u64;
    let mut accepted_births = 0_u64;
    let mut death_proposals = 0_u64;
    let mut accepted_deaths = 0_u64;
    let mut empty_death_transitions = 0_u64;
    let mut neighbor_visits = 0_u64;
    let mut maximum_observed_count = 0_u32;
    for _ in 0..spec.iterations {
        if rng.gen_bool(0.5) {
            birth_proposals += 1;
            if points.len() >= spec.maximum_points as usize {
                return Err(StraussBirthDeathError::Resource(
                    "birth proposal reached the declared point cap".into(),
                ));
            }
            add_visits(
                &mut neighbor_visits,
                points.len() as u64,
                spec.maximum_neighbor_visits,
            )?;
            let proposal = (
                rng.gen_range(spec.window.xmin_um..spec.window.xmax_um),
                rng.gen_range(spec.window.ymin_um..spec.window.ymax_um),
            );
            let delta = neighbor_delta(proposal, &points, spec.interaction_radius_um)
                .map_err(|error| StraussBirthDeathError::Numerical(error.to_string()))?;
            let lambda = papangelou_from_delta(spec.beta_per_um2, spec.gamma, delta);
            let ratio = area * lambda / (points.len() as f64 + 1.0);
            let acceptance = ratio.min(1.0);
            validate_acceptance(acceptance)?;
            if rng.gen::<f64>() < acceptance {
                points.push(proposal);
                accepted_births += 1;
            }
        } else {
            death_proposals += 1;
            if points.is_empty() {
                empty_death_transitions += 1;
            } else {
                let index = rng.gen_range(0..points.len());
                let removed = points.swap_remove(index);
                add_visits(
                    &mut neighbor_visits,
                    points.len() as u64,
                    spec.maximum_neighbor_visits,
                )?;
                let delta = neighbor_delta(removed, &points, spec.interaction_radius_um)
                    .map_err(|error| StraussBirthDeathError::Numerical(error.to_string()))?;
                let lambda = papangelou_from_delta(spec.beta_per_um2, spec.gamma, delta);
                let acceptance = if lambda == 0.0 {
                    1.0
                } else {
                    ((points.len() as f64 + 1.0) / (area * lambda)).min(1.0)
                };
                validate_acceptance(acceptance)?;
                if rng.gen::<f64>() < acceptance {
                    accepted_deaths += 1;
                } else {
                    points.push(removed);
                }
            }
        }
        maximum_observed_count = maximum_observed_count.max(points.len() as u32);
        trace.push(points.len() as u32);
    }
    let post_burn = &trace[spec.burn_in as usize + 1..];
    let split = post_burn.len() / 2;
    let overall = mean_count(post_burn);
    let first = mean_count(&post_burn[..split]);
    let second = mean_count(&post_burn[split..]);
    let final_pattern = points
        .into_iter()
        .enumerate()
        .map(|(index, (x_um, y_um))| StraussPoint {
            point_id: format!("point:{index}"),
            x_um,
            y_um,
        })
        .collect::<Vec<_>>();
    Ok(StraussBirthDeathResult {
        format: "marklab.strauss_birth_death_simulation",
        version: 1,
        coordinate_unit: "micrometer",
        window: spec.window,
        beta_per_um2: spec.beta_per_um2,
        gamma: spec.gamma,
        interaction_radius_um: spec.interaction_radius_um,
        pair_boundary: "euclidean_distance_less_than_or_equal_radius",
        iterations: spec.iterations,
        burn_in: spec.burn_in,
        seed: spec.seed,
        rng: "rand_chacha_0.3.1_chacha20",
        initial_state: "empty_pattern",
        transition_rule: "equal_probability_birth_death_metropolis_hastings",
        maximum_points: spec.maximum_points,
        maximum_neighbor_visits: spec.maximum_neighbor_visits,
        diagnostics: StraussBirthDeathDiagnostics {
            birth_proposals,
            accepted_births,
            death_proposals,
            accepted_deaths,
            empty_death_transitions,
            rejected_births: birth_proposals - accepted_births,
            rejected_deaths: death_proposals - accepted_deaths - empty_death_transitions,
            neighbor_visits,
            maximum_observed_count,
            final_count: final_pattern.len() as u32,
            post_burn_state_count: post_burn.len() as u32,
            post_burn_mean_count: overall,
            post_burn_first_half_mean_count: first,
            post_burn_second_half_mean_count: second,
            absolute_half_mean_drift: (first - second).abs(),
            poisson_special_case_expected_count: if spec.gamma == 1.0 {
                Some(spec.beta_per_um2 * area)
            } else {
                None
            },
        },
        count_trace: trace,
        final_pattern,
        claim_status: "experimental_finite_chain_simulation",
    })
}

fn validate(spec: &StraussBirthDeathSpec) -> Result<(), StraussBirthDeathError> {
    if ![
        spec.window.xmin_um,
        spec.window.ymin_um,
        spec.window.xmax_um,
        spec.window.ymax_um,
        spec.beta_per_um2,
        spec.gamma,
        spec.interaction_radius_um,
    ]
    .into_iter()
    .all(f64::is_finite)
        || spec.window.xmin_um >= spec.window.xmax_um
        || spec.window.ymin_um >= spec.window.ymax_um
        || spec.beta_per_um2 <= 0.0
        || !(0.0..=1.0).contains(&spec.gamma)
        || spec.interaction_radius_um <= 0.0
        || !(100..=100_000).contains(&spec.iterations)
        || spec.burn_in >= spec.iterations
        || !(1..=10_000).contains(&spec.maximum_points)
        || !(1..=100_000_000).contains(&spec.maximum_neighbor_visits)
    {
        return Err(StraussBirthDeathError::InvalidSpec(
            "Strauss window, parameters, iterations/burn-in, or resource caps are invalid".into(),
        ));
    }
    let post_burn = spec.iterations - spec.burn_in;
    if post_burn < 2 {
        return Err(StraussBirthDeathError::InvalidSpec(
            "Strauss birth/death requires at least two post-burn states".into(),
        ));
    }
    Ok(())
}

fn add_visits(total: &mut u64, add: u64, cap: u64) -> Result<(), StraussBirthDeathError> {
    *total = total
        .checked_add(add)
        .ok_or_else(|| StraussBirthDeathError::Resource("neighbor visits overflow".into()))?;
    if *total > cap {
        return Err(StraussBirthDeathError::Resource(
            "neighbor visits exceed the declared cap".into(),
        ));
    }
    Ok(())
}

fn validate_acceptance(value: f64) -> Result<(), StraussBirthDeathError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(StraussBirthDeathError::Numerical(
            "birth/death acceptance probability is invalid".into(),
        ));
    }
    Ok(())
}

fn mean_count(values: &[u32]) -> f64 {
    values.iter().map(|value| f64::from(*value)).sum::<f64>() / values.len() as f64
}
