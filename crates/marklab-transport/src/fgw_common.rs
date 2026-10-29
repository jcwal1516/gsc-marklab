use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FgwSupport {
    pub id: String,
    pub mass: f64,
    pub features: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FgwPlanEntry {
    pub source_id: String,
    pub target_id: String,
    pub mass: f64,
    pub feature_cost: f64,
}

pub(crate) struct PlanMarginals {
    pub(crate) source: Vec<f64>,
    pub(crate) target: Vec<f64>,
}

pub(crate) fn valid_support(
    rows: &[FgwSupport],
    feature_dimension: usize,
    mass_is_valid: impl Fn(f64) -> bool,
) -> bool {
    let mut ids = HashSet::new();
    rows.iter().all(|row| {
        !row.id.trim().is_empty()
            && row.id.trim() == row.id
            && ids.insert(row.id.as_str())
            && row.mass.is_finite()
            && row.mass > 0.0
            && mass_is_valid(row.mass)
            && row.features.len() == feature_dimension
            && row.features.iter().all(|value| value.is_finite())
    })
}

pub(crate) fn valid_structure(matrix: &[f64], size: usize) -> bool {
    (0..size).all(|row| {
        (0..size).all(|column| {
            let value = matrix[row * size + column];
            value.is_finite()
                && value >= 0.0
                && (row != column || value.abs() <= 1e-12)
                && approximately_equal(value, matrix[column * size + row])
        })
    })
}

pub(crate) fn audit_plan(
    plan: &[FgwPlanEntry],
    source: &[FgwSupport],
    target: &[FgwSupport],
    feature_scale: f64,
    mut observe: impl FnMut(&FgwPlanEntry, f64),
) -> Option<PlanMarginals> {
    let rows = source.len();
    let columns = target.len();
    if plan.len() != rows * columns {
        return None;
    }
    let mut source_marginals = vec![0.0; rows];
    let mut target_marginals = vec![0.0; columns];
    for (row, source_marginal) in source_marginals.iter_mut().enumerate() {
        for (column, target_marginal) in target_marginals.iter_mut().enumerate() {
            let entry = &plan[row * columns + column];
            let feature_cost = squared_feature_cost(
                &source[row].features,
                &target[column].features,
                feature_scale,
            );
            if entry.source_id != source[row].id
                || entry.target_id != target[column].id
                || !approximately_equal(entry.feature_cost, feature_cost)
                || !entry.mass.is_finite()
                || entry.mass < 0.0
            {
                return None;
            }
            *source_marginal += entry.mass;
            *target_marginal += entry.mass;
            observe(entry, feature_cost);
        }
    }
    Some(PlanMarginals {
        source: source_marginals,
        target: target_marginals,
    })
}

pub(crate) fn squared_feature_cost(source: &[f64], target: &[f64], scale: f64) -> f64 {
    source
        .iter()
        .zip(target)
        .map(|(left, right)| ((left - right) / scale).powi(2))
        .sum()
}

pub(crate) fn structural_objective(
    plan: &[FgwPlanEntry],
    rows: usize,
    columns: usize,
    source_structure_row_major: &[f64],
    target_structure_row_major: &[f64],
    structure_scale: f64,
) -> f64 {
    let mut objective = 0.0;
    for source_left in 0..rows {
        for target_left in 0..columns {
            for source_right in 0..rows {
                for target_right in 0..columns {
                    let source_distance = source_structure_row_major
                        [source_left * rows + source_right]
                        / structure_scale;
                    let target_distance = target_structure_row_major
                        [target_left * columns + target_right]
                        / structure_scale;
                    objective += (source_distance - target_distance).powi(2)
                        * plan[source_left * columns + target_left].mass
                        * plan[source_right * columns + target_right].mass;
                }
            }
        }
    }
    objective
}

pub(crate) fn same_plan(left: &[FgwPlanEntry], right: &[FgwPlanEntry]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.source_id == right.source_id
                && left.target_id == right.target_id
                && approximately_equal(left.mass, right.mass)
                && approximately_equal(left.feature_cost, right.feature_cost)
        })
}

pub(crate) fn same_values(left: &[f64], right: &[f64]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| approximately_equal(*left, *right))
}

pub(crate) fn approximately_equal(left: f64, right: f64) -> bool {
    marklab_numerics::relative_eq(left, right, 1e-8)
}
