use super::{admission::ComponentGraph, model::SparseRadiusBasisMode};
use crate::{
    sparse_radius_graph::laplacian_product, symmetric_eigendecomposition_with_limit, GraphError,
};

const SUBSPACE_STEP: f64 = 0.99;

pub(super) struct CandidateMode {
    pub(super) component_index: usize,
    pub(super) eigenvalue: f64,
    pub(super) residual_l2: f64,
    pub(super) zero_mode: bool,
    pub(super) local_values: Vec<f64>,
}

pub(super) fn approximate_component_modes(
    component_index: usize,
    component: &ComponentGraph,
    mode_count: usize,
    iterations: usize,
    maximum_rotations: usize,
) -> Result<(Vec<CandidateMode>, usize), GraphError> {
    let size = component.global_nodes.len();
    let mut basis = (0..mode_count)
        .map(|column| {
            let mut values = vec![-1.0 / size as f64; size];
            values[column] += 1.0;
            values
        })
        .collect::<Vec<_>>();
    orthonormalize_zero_sum(&mut basis)?;
    let maximum_degree = component.degrees.iter().copied().max().unwrap_or(0);
    let spectral_upper_bound = 2.0 * f64::from(maximum_degree);
    let scale = SUBSPACE_STEP / spectral_upper_bound;
    for _ in 0..iterations {
        for vector in &mut basis {
            let laplacian = laplacian_product(&component.degrees, &component.edges, vector);
            for (value, applied) in vector.iter_mut().zip(laplacian) {
                *value -= scale * applied;
            }
        }
        orthonormalize_zero_sum(&mut basis)?;
    }
    let applied = basis
        .iter()
        .map(|vector| laplacian_product(&component.degrees, &component.edges, vector))
        .collect::<Vec<_>>();
    let mut projected = vec![vec![0.0; mode_count]; mode_count];
    for left in 0..mode_count {
        for right in left..mode_count {
            let value = dot(&basis[left], &applied[right]);
            let reverse = dot(&basis[right], &applied[left]);
            projected[left][right] = 0.5 * (value + reverse);
            projected[right][left] = projected[left][right];
        }
    }
    let (eigenvalues, coefficients, rotations) = if mode_count == 1 {
        (vec![projected[0][0]], vec![vec![1.0]], 0)
    } else {
        let decomposition = symmetric_eigendecomposition_with_limit(&projected, maximum_rotations)?;
        (
            decomposition.values,
            decomposition.vectors,
            decomposition.rotations,
        )
    };
    let mut modes = Vec::with_capacity(mode_count);
    for (eigenvalue, coefficient) in eigenvalues.into_iter().zip(coefficients) {
        let mut values = linear_combination(&basis, &coefficient);
        let applied_values = linear_combination(&applied, &coefficient);
        let residual_l2 = applied_values
            .iter()
            .zip(&values)
            .map(|(applied, value)| (applied - eigenvalue * value).powi(2))
            .sum::<f64>()
            .sqrt();
        if !eigenvalue.is_finite()
            || eigenvalue < -1e-10
            || !residual_l2.is_finite()
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(GraphError::Numerical(
                "sparse basis Ritz result is non-finite or negative".into(),
            ));
        }
        orient_mode(&mut values);
        modes.push(CandidateMode {
            component_index,
            eigenvalue: eigenvalue.max(0.0),
            residual_l2,
            zero_mode: false,
            local_values: values,
        });
    }
    Ok((modes, rotations))
}

fn orthonormalize_zero_sum(columns: &mut [Vec<f64>]) -> Result<(), GraphError> {
    for column in 0..columns.len() {
        subtract_mean(&mut columns[column]);
        for _ in 0..2 {
            let (previous, current) = columns.split_at_mut(column);
            let current = &mut current[0];
            for vector in previous {
                let projection = dot(current, vector);
                for (value, basis) in current.iter_mut().zip(vector) {
                    *value -= projection * *basis;
                }
            }
        }
        subtract_mean(&mut columns[column]);
        let norm = dot(&columns[column], &columns[column]).sqrt();
        if !norm.is_finite() || norm <= 1e-14 {
            return Err(GraphError::Numerical(
                "sparse basis subspace lost numerical rank".into(),
            ));
        }
        for value in &mut columns[column] {
            *value /= norm;
        }
    }
    Ok(())
}

fn subtract_mean(values: &mut [f64]) {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    for value in values {
        *value -= mean;
    }
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn linear_combination(columns: &[Vec<f64>], coefficients: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; columns[0].len()];
    for (column, coefficient) in columns.iter().zip(coefficients) {
        for (value, basis) in result.iter_mut().zip(column) {
            *value += coefficient * basis;
        }
    }
    result
}

pub(super) fn orient_mode(values: &mut [f64]) {
    if values
        .iter()
        .find(|value| value.abs() > 1e-12)
        .is_some_and(|value| *value < 0.0)
    {
        for value in values {
            *value = -*value;
        }
    }
}

pub(super) fn orthogonality_error(modes: &[SparseRadiusBasisMode]) -> f64 {
    let mut maximum = 0.0_f64;
    for (left, left_mode) in modes.iter().enumerate() {
        for (right, right_mode) in modes.iter().enumerate().skip(left) {
            let expected = f64::from(left == right);
            let observed = if left_mode.component_index == right_mode.component_index {
                dot(
                    &left_mode.values_by_component_node,
                    &right_mode.values_by_component_node,
                )
            } else {
                0.0
            };
            maximum = maximum.max((observed - expected).abs());
        }
    }
    maximum
}
