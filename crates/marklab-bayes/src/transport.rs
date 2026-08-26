use std::collections::HashSet;

use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Debug, Serialize)]
pub struct TransportMass {
    pub id: String,
    pub mass: f64,
}

#[derive(Clone, Debug)]
pub struct SinkhornSpec {
    pub source: Vec<TransportMass>,
    pub target: Vec<TransportMass>,
    pub costs_row_major: Vec<f64>,
    pub epsilon: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct TransportPlanEntry {
    pub source_id: String,
    pub target_id: String,
    pub mass: f64,
    pub cost: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SinkhornResult {
    pub format: &'static str,
    pub version: u32,
    pub algorithm: &'static str,
    pub epsilon: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
    pub iterations: u32,
    pub converged: bool,
    pub source_ids: Vec<String>,
    pub target_ids: Vec<String>,
    pub source_masses: Vec<f64>,
    pub target_masses: Vec<f64>,
    pub source_dual_potentials: Vec<Option<f64>>,
    pub target_dual_potentials: Vec<Option<f64>>,
    pub plan: Vec<TransportPlanEntry>,
    pub source_marginals: Vec<f64>,
    pub target_marginals: Vec<f64>,
    pub maximum_marginal_residual: f64,
    pub transport_cost: f64,
    pub entropy: f64,
    pub regularized_cost: f64,
    pub zero_mass_policy: &'static str,
}

#[derive(Clone, Debug)]
pub struct EntropicSoftAssignmentSpec {
    pub source: Vec<TransportMass>,
    pub target: Vec<TransportMass>,
    pub costs_row_major: Vec<f64>,
    pub epsilon: f64,
    pub dustbin_cost: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntropicSoftAssignmentSensitivity {
    pub epsilon: f64,
    pub converged: bool,
    pub real_transported_mass: f64,
    pub unmatched_source_total: f64,
    pub unmatched_target_total: f64,
    pub regularized_cost: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntropicSoftAssignmentResult {
    pub format: &'static str,
    pub version: u32,
    pub algorithm: &'static str,
    pub epsilon: f64,
    pub dustbin_cost: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
    pub iterations: u32,
    pub converged: bool,
    pub real_plan: Vec<TransportPlanEntry>,
    pub unmatched_source_mass: Vec<f64>,
    pub unmatched_target_mass: Vec<f64>,
    pub dustbin_to_dustbin_mass: f64,
    pub maximum_marginal_residual: f64,
    pub transport_cost: f64,
    pub entropy: f64,
    pub regularized_cost: f64,
    pub epsilon_sensitivity: Vec<EntropicSoftAssignmentSensitivity>,
    pub claim_status: &'static str,
}

#[derive(Clone, Debug)]
pub struct UnbalancedSinkhornSpec {
    pub source: Vec<TransportMass>,
    pub target: Vec<TransportMass>,
    pub costs_row_major: Vec<f64>,
    pub epsilon: f64,
    pub tau_source: f64,
    pub tau_target: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnbalancedSinkhornResult {
    pub format: &'static str,
    pub version: u32,
    pub algorithm: &'static str,
    pub epsilon: f64,
    pub tau_source: f64,
    pub tau_target: f64,
    pub source_exponent: f64,
    pub target_exponent: f64,
    pub tolerance: f64,
    pub maximum_iterations: u32,
    pub iterations: u32,
    pub converged: bool,
    pub final_scaling_residual: f64,
    pub plan: Vec<TransportPlanEntry>,
    pub source_marginals: Vec<f64>,
    pub target_marginals: Vec<f64>,
    pub transported_mass: f64,
    pub source_marginal_deviations: Vec<f64>,
    pub target_marginal_deviations: Vec<f64>,
    pub transport_cost: f64,
    pub entropy: f64,
    pub source_kl_divergence: f64,
    pub target_kl_divergence: f64,
    pub objective: f64,
    pub zero_mass_policy: &'static str,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("invalid transport input: {0}")]
    Invalid(String),
    #[error("transport numerical failure: {0}")]
    Numerical(String),
}

pub fn entropic_soft_assignment(
    spec: EntropicSoftAssignmentSpec,
) -> Result<EntropicSoftAssignmentResult, TransportError> {
    validate_soft_assignment_spec(&spec)?;
    let rows = spec.source.len();
    let columns = spec.target.len();
    let source_total = spec.source.iter().map(|row| row.mass).sum::<f64>();
    let target_total = spec.target.iter().map(|row| row.mass).sum::<f64>();
    let mut source = spec.source.clone();
    source.push(TransportMass {
        id: "__dustbin_source__".into(),
        mass: target_total,
    });
    let mut target = spec.target.clone();
    target.push(TransportMass {
        id: "__dustbin_target__".into(),
        mass: source_total,
    });
    let augmented_columns = columns + 1;
    let mut costs = Vec::with_capacity((rows + 1) * augmented_columns);
    for row in 0..=rows {
        for column in 0..=columns {
            costs.push(if row < rows && column < columns {
                spec.costs_row_major[row * columns + column]
            } else {
                spec.dustbin_cost
            });
        }
    }

    let mut sensitivity = Vec::with_capacity(3);
    let mut base = None;
    for epsilon in [spec.epsilon / 2.0, spec.epsilon, spec.epsilon * 2.0] {
        let fit = sinkhorn_ot(SinkhornSpec {
            source: source.clone(),
            target: target.clone(),
            costs_row_major: costs.clone(),
            epsilon,
            tolerance: spec.tolerance,
            maximum_iterations: spec.maximum_iterations,
        })?;
        let real_transported_mass = real_plan_mass(&fit, rows, columns, augmented_columns);
        let unmatched_source_total = (0..rows)
            .map(|row| fit.plan[row * augmented_columns + columns].mass)
            .sum::<f64>();
        let unmatched_target_total = (0..columns)
            .map(|column| fit.plan[rows * augmented_columns + column].mass)
            .sum::<f64>();
        sensitivity.push(EntropicSoftAssignmentSensitivity {
            epsilon,
            converged: fit.converged,
            real_transported_mass: canonical_zero(real_transported_mass),
            unmatched_source_total: canonical_zero(unmatched_source_total),
            unmatched_target_total: canonical_zero(unmatched_target_total),
            regularized_cost: fit.regularized_cost,
        });
        if epsilon == spec.epsilon {
            base = Some(fit);
        }
    }
    let base = base.expect("base epsilon is always evaluated");
    let real_plan = (0..rows)
        .flat_map(|row| {
            let base = &base;
            (0..columns).map(move |column| base.plan[row * augmented_columns + column].clone())
        })
        .collect();
    let unmatched_source_mass = (0..rows)
        .map(|row| base.plan[row * augmented_columns + columns].mass)
        .collect();
    let unmatched_target_mass = (0..columns)
        .map(|column| base.plan[rows * augmented_columns + column].mass)
        .collect();
    Ok(EntropicSoftAssignmentResult {
        format: "marklab.entropic_soft_assignment",
        version: 1,
        algorithm: "balanced_log_domain_sinkhorn_with_explicit_dustbins",
        epsilon: spec.epsilon,
        dustbin_cost: spec.dustbin_cost,
        tolerance: spec.tolerance,
        maximum_iterations: spec.maximum_iterations,
        iterations: base.iterations,
        converged: base.converged,
        real_plan,
        unmatched_source_mass,
        unmatched_target_mass,
        dustbin_to_dustbin_mass: base.plan[rows * augmented_columns + columns].mass,
        maximum_marginal_residual: base.maximum_marginal_residual,
        transport_cost: base.transport_cost,
        entropy: base.entropy,
        regularized_cost: base.regularized_cost,
        epsilon_sensitivity: sensitivity,
        claim_status: "probabilistic_compatibility_not_cell_identity",
    })
}

fn validate_soft_assignment_spec(spec: &EntropicSoftAssignmentSpec) -> Result<(), TransportError> {
    let source_total = spec.source.iter().map(|row| row.mass).sum::<f64>();
    let target_total = spec.target.iter().map(|row| row.mass).sum::<f64>();
    let augmented_work = (spec.source.len() as u64 + 1)
        .saturating_mul(spec.target.len() as u64 + 1)
        .saturating_mul(u64::from(spec.maximum_iterations))
        .saturating_mul(3);
    if spec.source.iter().any(|row| row.id == "__dustbin_source__")
        || spec.target.iter().any(|row| row.id == "__dustbin_target__")
        || !(spec.dustbin_cost.is_finite() && spec.dustbin_cost >= 0.0)
        || !(spec.epsilon.is_finite() && spec.epsilon > 0.0 && spec.epsilon <= f64::MAX / 2.0)
        || !source_total.is_finite()
        || !target_total.is_finite()
        || source_total <= 0.0
        || target_total <= 0.0
        || augmented_work > 250_000_000
    {
        return Err(TransportError::Invalid(
            "soft assignment masses, controls, reserved identities, or work bound are invalid"
                .into(),
        ));
    }
    Ok(())
}

fn real_plan_mass(
    result: &SinkhornResult,
    rows: usize,
    columns: usize,
    augmented_columns: usize,
) -> f64 {
    (0..rows)
        .flat_map(|row| {
            (0..columns).map(move |column| result.plan[row * augmented_columns + column].mass)
        })
        .sum()
}

pub fn sinkhorn_ot(spec: SinkhornSpec) -> Result<SinkhornResult, TransportError> {
    validate_spec(&spec)?;
    let rows = spec.source.len();
    let columns = spec.target.len();
    let active_rows = spec
        .source
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.mass > 0.0).then_some(index))
        .collect::<Vec<_>>();
    let active_columns = spec
        .target
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.mass > 0.0).then_some(index))
        .collect::<Vec<_>>();
    let mut source_potential = vec![0.0; rows];
    let mut target_potential = vec![0.0; columns];
    let mut iterations = 0;
    let mut converged = false;

    for iteration in 1..=spec.maximum_iterations {
        iterations = iteration;
        for &row in &active_rows {
            let terms = active_columns.iter().map(|column| {
                (target_potential[*column] - spec.costs_row_major[row * columns + *column])
                    / spec.epsilon
            });
            source_potential[row] =
                spec.epsilon * (spec.source[row].mass.ln() - log_sum_exp(terms));
        }
        for &column in &active_columns {
            let terms = active_rows.iter().map(|row| {
                (source_potential[*row] - spec.costs_row_major[*row * columns + column])
                    / spec.epsilon
            });
            target_potential[column] =
                spec.epsilon * (spec.target[column].mass.ln() - log_sum_exp(terms));
        }
        if iteration.is_multiple_of(20) {
            let shift = active_rows
                .iter()
                .map(|row| source_potential[*row])
                .sum::<f64>()
                / active_rows.len() as f64;
            for &row in &active_rows {
                source_potential[row] -= shift;
            }
            for &column in &active_columns {
                target_potential[column] += shift;
            }
        }
        let maximum_residual = marginal_residual(
            &spec,
            &source_potential,
            &target_potential,
            &active_rows,
            &active_columns,
        );
        if !maximum_residual.is_finite() {
            return Err(TransportError::Numerical(
                "marginal residual became non-finite".into(),
            ));
        }
        if maximum_residual <= spec.tolerance {
            converged = true;
            break;
        }
    }

    let mut plan = Vec::with_capacity(rows * columns);
    let mut source_marginals = vec![0.0; rows];
    let mut target_marginals = vec![0.0; columns];
    let mut transport_cost = 0.0;
    let mut entropy = 0.0;
    let mut negative_entropy = 0.0;
    for row in 0..rows {
        for column in 0..columns {
            let mass = if spec.source[row].mass == 0.0 || spec.target[column].mass == 0.0 {
                0.0
            } else {
                ((source_potential[row] + target_potential[column]
                    - spec.costs_row_major[row * columns + column])
                    / spec.epsilon)
                    .exp()
            };
            if !mass.is_finite() || mass < 0.0 {
                return Err(TransportError::Numerical(
                    "transport plan contains invalid mass".into(),
                ));
            }
            source_marginals[row] += mass;
            target_marginals[column] += mass;
            transport_cost += mass * spec.costs_row_major[row * columns + column];
            if mass > 0.0 {
                entropy -= mass * mass.ln();
                negative_entropy += mass * (mass.ln() - 1.0);
            }
            plan.push(TransportPlanEntry {
                source_id: spec.source[row].id.clone(),
                target_id: spec.target[column].id.clone(),
                mass: canonical_zero(mass),
                cost: spec.costs_row_major[row * columns + column],
            });
        }
    }
    let maximum_residual = source_marginals
        .iter()
        .zip(&spec.source)
        .map(|(actual, expected)| (actual - expected.mass).abs())
        .chain(
            target_marginals
                .iter()
                .zip(&spec.target)
                .map(|(actual, expected)| (actual - expected.mass).abs()),
        )
        .fold(0.0, f64::max);
    let regularized_cost = transport_cost + spec.epsilon * negative_entropy;
    if [transport_cost, entropy, regularized_cost, maximum_residual]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(TransportError::Numerical(
            "transport result contains a non-finite scalar".into(),
        ));
    }
    Ok(SinkhornResult {
        format: "marklab.sinkhorn_ot",
        version: 1,
        algorithm: "balanced_log_domain_sinkhorn",
        epsilon: spec.epsilon,
        tolerance: spec.tolerance,
        maximum_iterations: spec.maximum_iterations,
        iterations,
        converged,
        source_ids: spec.source.iter().map(|row| row.id.clone()).collect(),
        target_ids: spec.target.iter().map(|row| row.id.clone()).collect(),
        source_masses: spec.source.iter().map(|row| row.mass).collect(),
        target_masses: spec.target.iter().map(|row| row.mass).collect(),
        source_dual_potentials: spec
            .source
            .iter()
            .enumerate()
            .map(|(index, row)| (row.mass > 0.0).then_some(source_potential[index]))
            .collect(),
        target_dual_potentials: spec
            .target
            .iter()
            .enumerate()
            .map(|(index, row)| (row.mass > 0.0).then_some(target_potential[index]))
            .collect(),
        plan,
        source_marginals: source_marginals.into_iter().map(canonical_zero).collect(),
        target_marginals: target_marginals.into_iter().map(canonical_zero).collect(),
        maximum_marginal_residual: canonical_zero(maximum_residual),
        transport_cost: canonical_zero(transport_cost),
        entropy: canonical_zero(entropy),
        regularized_cost: canonical_zero(regularized_cost),
        zero_mass_policy: "remove_from_dual_updates_and_restore_zero_plan_rows_or_columns",
    })
}

pub fn unbalanced_sinkhorn(
    spec: UnbalancedSinkhornSpec,
) -> Result<UnbalancedSinkhornResult, TransportError> {
    validate_unbalanced_spec(&spec)?;
    let rows = spec.source.len();
    let columns = spec.target.len();
    let active_rows = spec
        .source
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.mass > 0.0).then_some(index))
        .collect::<Vec<_>>();
    let active_columns = spec
        .target
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.mass > 0.0).then_some(index))
        .collect::<Vec<_>>();
    let source_exponent = spec.tau_source / (spec.tau_source + spec.epsilon);
    let target_exponent = spec.tau_target / (spec.tau_target + spec.epsilon);
    let mut log_source_scaling = vec![0.0; rows];
    let mut log_target_scaling = vec![0.0; columns];
    let mut iterations = 0;
    let mut converged = false;
    let mut final_scaling_residual = f64::INFINITY;
    for iteration in 1..=spec.maximum_iterations {
        iterations = iteration;
        let previous_source = log_source_scaling.clone();
        let previous_target = log_target_scaling.clone();
        for &row in &active_rows {
            let product = log_sum_exp(active_columns.iter().map(|column| {
                log_target_scaling[*column]
                    - spec.costs_row_major[row * columns + *column] / spec.epsilon
            }));
            log_source_scaling[row] = source_exponent * (spec.source[row].mass.ln() - product);
        }
        for &column in &active_columns {
            let product = log_sum_exp(active_rows.iter().map(|row| {
                log_source_scaling[*row]
                    - spec.costs_row_major[*row * columns + column] / spec.epsilon
            }));
            log_target_scaling[column] =
                target_exponent * (spec.target[column].mass.ln() - product);
        }
        final_scaling_residual = active_rows
            .iter()
            .map(|row| (log_source_scaling[*row] - previous_source[*row]).abs())
            .chain(
                active_columns
                    .iter()
                    .map(|column| (log_target_scaling[*column] - previous_target[*column]).abs()),
            )
            .fold(0.0, f64::max);
        if !final_scaling_residual.is_finite() {
            return Err(TransportError::Numerical(
                "unbalanced scaling residual became non-finite".into(),
            ));
        }
        if final_scaling_residual <= spec.tolerance {
            converged = true;
            break;
        }
    }

    let mut plan = Vec::with_capacity(rows * columns);
    let mut source_marginals = vec![0.0; rows];
    let mut target_marginals = vec![0.0; columns];
    let mut transport_cost = 0.0;
    let mut entropy = 0.0;
    let mut negative_entropy = 0.0;
    for row in 0..rows {
        for column in 0..columns {
            let mass = if spec.source[row].mass == 0.0 || spec.target[column].mass == 0.0 {
                0.0
            } else {
                (log_source_scaling[row] + log_target_scaling[column]
                    - spec.costs_row_major[row * columns + column] / spec.epsilon)
                    .exp()
            };
            if !mass.is_finite() || mass < 0.0 {
                return Err(TransportError::Numerical(
                    "unbalanced plan contains invalid mass".into(),
                ));
            }
            source_marginals[row] += mass;
            target_marginals[column] += mass;
            transport_cost += mass * spec.costs_row_major[row * columns + column];
            if mass > 0.0 {
                entropy -= mass * mass.ln();
                negative_entropy += mass * (mass.ln() - 1.0);
            }
            plan.push(TransportPlanEntry {
                source_id: spec.source[row].id.clone(),
                target_id: spec.target[column].id.clone(),
                mass: canonical_zero(mass),
                cost: spec.costs_row_major[row * columns + column],
            });
        }
    }
    let source_kl_divergence = generalized_kl(&source_marginals, &spec.source);
    let target_kl_divergence = generalized_kl(&target_marginals, &spec.target);
    let transported_mass = source_marginals.iter().sum::<f64>();
    let objective = transport_cost
        + spec.epsilon * negative_entropy
        + spec.tau_source * source_kl_divergence
        + spec.tau_target * target_kl_divergence;
    if [
        transported_mass,
        transport_cost,
        entropy,
        source_kl_divergence,
        target_kl_divergence,
        objective,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(TransportError::Numerical(
            "unbalanced result contains a non-finite scalar".into(),
        ));
    }
    Ok(UnbalancedSinkhornResult {
        format: "marklab.unbalanced_sinkhorn",
        version: 1,
        algorithm: "log_domain_kl_unbalanced_sinkhorn",
        epsilon: spec.epsilon,
        tau_source: spec.tau_source,
        tau_target: spec.tau_target,
        source_exponent,
        target_exponent,
        tolerance: spec.tolerance,
        maximum_iterations: spec.maximum_iterations,
        iterations,
        converged,
        final_scaling_residual,
        plan,
        source_marginal_deviations: source_marginals
            .iter()
            .zip(&spec.source)
            .map(|(actual, expected)| canonical_zero(actual - expected.mass))
            .collect(),
        target_marginal_deviations: target_marginals
            .iter()
            .zip(&spec.target)
            .map(|(actual, expected)| canonical_zero(actual - expected.mass))
            .collect(),
        source_marginals: source_marginals.into_iter().map(canonical_zero).collect(),
        target_marginals: target_marginals.into_iter().map(canonical_zero).collect(),
        transported_mass,
        transport_cost,
        entropy,
        source_kl_divergence,
        target_kl_divergence,
        objective,
        zero_mass_policy: "remove_from_scaling_updates_and_restore_zero_plan_rows_or_columns",
    })
}

fn validate_unbalanced_spec(spec: &UnbalancedSinkhornSpec) -> Result<(), TransportError> {
    if !((1..=2_000).contains(&spec.source.len())
        && (1..=2_000).contains(&spec.target.len())
        && spec.costs_row_major.len() == spec.source.len() * spec.target.len()
        && spec.epsilon.is_finite()
        && spec.epsilon > 0.0)
        || !(spec.tau_source.is_finite() && spec.tau_source > 0.0)
        || !(spec.tau_target.is_finite() && spec.tau_target > 0.0)
        || !(spec.tolerance.is_finite() && spec.tolerance > 0.0 && spec.tolerance <= 1.0)
        || !(1..=1_000_000).contains(&spec.maximum_iterations)
    {
        return Err(TransportError::Invalid(
            "unbalanced transport dimensions or controls are invalid".into(),
        ));
    }
    let mut source_ids = HashSet::new();
    let mut target_ids = HashSet::new();
    if spec.source.iter().any(|row| {
        row.id.trim().is_empty()
            || row.id.trim() != row.id
            || !source_ids.insert(row.id.as_str())
            || !row.mass.is_finite()
            || row.mass < 0.0
    }) || spec.target.iter().any(|row| {
        row.id.trim().is_empty()
            || row.id.trim() != row.id
            || !target_ids.insert(row.id.as_str())
            || !row.mass.is_finite()
            || row.mass < 0.0
    }) || spec
        .costs_row_major
        .iter()
        .any(|cost| !cost.is_finite() || *cost < 0.0)
        || spec.source.iter().all(|row| row.mass == 0.0)
        || spec.target.iter().all(|row| row.mass == 0.0)
    {
        return Err(TransportError::Invalid(
            "unbalanced transport identities, masses, or costs are invalid".into(),
        ));
    }
    let work =
        spec.source.len() as u64 * spec.target.len() as u64 * u64::from(spec.maximum_iterations);
    if work > 250_000_000 {
        return Err(TransportError::Invalid(
            "unbalanced transport work exceeds its bound".into(),
        ));
    }
    Ok(())
}

fn generalized_kl(actual: &[f64], expected: &[TransportMass]) -> f64 {
    actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| {
            if *actual == 0.0 {
                expected.mass
            } else {
                actual * (actual / expected.mass).ln() - actual + expected.mass
            }
        })
        .sum()
}

fn validate_spec(spec: &SinkhornSpec) -> Result<(), TransportError> {
    if !((1..=2_000).contains(&spec.source.len())
        && (1..=2_000).contains(&spec.target.len())
        && spec.costs_row_major.len() == spec.source.len() * spec.target.len()
        && spec.epsilon.is_finite()
        && spec.epsilon > 0.0)
        || !(spec.tolerance.is_finite() && spec.tolerance > 0.0 && spec.tolerance <= 1.0)
        || !(1..=1_000_000).contains(&spec.maximum_iterations)
    {
        return Err(TransportError::Invalid(
            "transport dimensions, epsilon, tolerance, or iterations are invalid".into(),
        ));
    }
    let mut source_ids = HashSet::new();
    let mut target_ids = HashSet::new();
    if spec.source.iter().any(|row| {
        row.id.trim().is_empty()
            || row.id.trim() != row.id
            || !source_ids.insert(row.id.as_str())
            || !row.mass.is_finite()
            || row.mass < 0.0
    }) || spec.target.iter().any(|row| {
        row.id.trim().is_empty()
            || row.id.trim() != row.id
            || !target_ids.insert(row.id.as_str())
            || !row.mass.is_finite()
            || row.mass < 0.0
    }) || spec
        .costs_row_major
        .iter()
        .any(|cost| !cost.is_finite() || *cost < 0.0)
    {
        return Err(TransportError::Invalid(
            "transport identities, masses, or costs are invalid".into(),
        ));
    }
    let source_total = spec.source.iter().map(|row| row.mass).sum::<f64>();
    let target_total = spec.target.iter().map(|row| row.mass).sum::<f64>();
    if source_total <= 0.0
        || target_total <= 0.0
        || (source_total - target_total).abs()
            > 1e-12 * (1.0 + source_total.abs().max(target_total.abs()))
    {
        return Err(TransportError::Invalid(
            "balanced transport requires equal positive total mass".into(),
        ));
    }
    let work =
        spec.source.len() as u64 * spec.target.len() as u64 * u64::from(spec.maximum_iterations);
    if work > 250_000_000 {
        return Err(TransportError::Invalid(
            "transport iteration work exceeds its bound".into(),
        ));
    }
    Ok(())
}

fn marginal_residual(
    spec: &SinkhornSpec,
    source_potential: &[f64],
    target_potential: &[f64],
    active_rows: &[usize],
    active_columns: &[usize],
) -> f64 {
    let columns = spec.target.len();
    let source_maximum = active_rows
        .iter()
        .map(|row| {
            let mass = active_columns
                .iter()
                .map(|column| {
                    ((source_potential[*row] + target_potential[*column]
                        - spec.costs_row_major[*row * columns + *column])
                        / spec.epsilon)
                        .exp()
                })
                .sum::<f64>();
            (mass - spec.source[*row].mass).abs()
        })
        .fold(0.0, f64::max);
    let target_maximum = active_columns
        .iter()
        .map(|column| {
            let mass = active_rows
                .iter()
                .map(|row| {
                    ((source_potential[*row] + target_potential[*column]
                        - spec.costs_row_major[*row * columns + *column])
                        / spec.epsilon)
                        .exp()
                })
                .sum::<f64>();
            (mass - spec.target[*column].mass).abs()
        })
        .fold(0.0, f64::max);
    source_maximum.max(target_maximum)
}

fn log_sum_exp(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    maximum
        + values
            .iter()
            .map(|value| (value - maximum).exp())
            .sum::<f64>()
            .ln()
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}
