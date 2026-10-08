use thiserror::Error;

use crate::{linalg, DiagonalPolicy, NormalizationPolicy, SymmetryPolicy, ValidatedSpatialWeights};

#[derive(Clone, Debug)]
pub struct IcarPlan {
    pub dimension: usize,
    pub constrained_dimension: usize,
    pub rank_deficiency: usize,
    pub components: Vec<Vec<usize>>,
    pub transform: Vec<f64>,
    pub typical_marginal_variance: f64,
    pub scaled_transform: Vec<f64>,
    pub scaled_typical_marginal_variance: f64,
}

#[derive(Debug, Error)]
pub enum IcarPlanError {
    #[error("invalid ICAR input: {0}")]
    InvalidInput(String),
    #[error("ICAR numerical failure: {0}")]
    Numerical(String),
}

pub fn build_icar_plan(weights: &ValidatedSpatialWeights) -> Result<IcarPlan, IcarPlanError> {
    let dimension = weights.region_ids.len();
    if !(6..=64).contains(&dimension) {
        return Err(IcarPlanError::InvalidInput(
            "ICAR plan requires 6-64 regions".into(),
        ));
    }
    if weights.policy.symmetry != SymmetryPolicy::Required
        || weights.policy.diagonal != DiagonalPolicy::Zero
        || weights.policy.normalization != NormalizationPolicy::Preserve
        || !weights.islands.is_empty()
        || weights
            .weights
            .iter()
            .any(|edge| edge.weight.to_bits() != 1.0_f64.to_bits())
    {
        return Err(IcarPlanError::InvalidInput(
            "ICAR requires island-free preserved symmetric binary zero-diagonal weights".into(),
        ));
    }
    let mut components = weights.components.clone();
    for component in &mut components {
        component.sort_unstable();
    }
    components.sort_by_key(|component| component[0]);
    let rank_deficiency = components.len();
    let constrained_dimension = dimension - rank_deficiency;
    let basis = component_helmert_basis(dimension, &components);
    let laplacian = laplacian(weights, dimension);
    let projected = project(&laplacian, dimension, &basis, constrained_dimension);
    let lower = linalg::cholesky(&projected, constrained_dimension).ok_or_else(|| {
        IcarPlanError::Numerical(
            "ICAR Laplacian is not positive definite on its constrained subspace".into(),
        )
    })?;
    let mut transform = vec![0.0; dimension * constrained_dimension];
    for column in 0..constrained_dimension {
        let mut coordinates = vec![0.0; constrained_dimension];
        coordinates[column] = 1.0;
        linalg::solve_upper_from_lower_transpose_in_place(
            &lower,
            constrained_dimension,
            &mut coordinates,
        );
        for row in 0..dimension {
            transform[row * constrained_dimension + column] = (0..constrained_dimension)
                .map(|inner| basis[row * constrained_dimension + inner] * coordinates[inner])
                .sum();
        }
    }
    if transform.iter().any(|value| !value.is_finite()) {
        return Err(IcarPlanError::Numerical(
            "ICAR transform contains a non-finite value".into(),
        ));
    }
    let marginal_variances = (0..dimension)
        .map(|row| {
            (0..constrained_dimension)
                .map(|column| transform[row * constrained_dimension + column].powi(2))
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    if marginal_variances
        .iter()
        .any(|variance| !variance.is_finite() || *variance <= 0.0)
    {
        return Err(IcarPlanError::Numerical(
            "ICAR generalized marginal variance is nonpositive or non-finite".into(),
        ));
    }
    let typical_marginal_variance = (marginal_variances
        .iter()
        .map(|variance| variance.ln())
        .sum::<f64>()
        / dimension as f64)
        .exp();
    let scale = typical_marginal_variance.sqrt().recip();
    let scaled_transform = transform
        .iter()
        .map(|value| value * scale)
        .collect::<Vec<_>>();
    let scaled_typical_marginal_variance = (marginal_variances
        .iter()
        .map(|variance| (variance * scale * scale).ln())
        .sum::<f64>()
        / dimension as f64)
        .exp();
    Ok(IcarPlan {
        dimension,
        constrained_dimension,
        rank_deficiency,
        components,
        transform,
        typical_marginal_variance,
        scaled_transform,
        scaled_typical_marginal_variance,
    })
}

fn component_helmert_basis(dimension: usize, components: &[Vec<usize>]) -> Vec<f64> {
    let constrained_dimension = dimension - components.len();
    let mut basis = vec![0.0; dimension * constrained_dimension];
    let mut output_column = 0;
    for component in components {
        for local_column in 0..component.len() - 1 {
            let denominator = (((local_column + 1) * (local_column + 2)) as f64).sqrt();
            for &row in &component[..=local_column] {
                basis[row * constrained_dimension + output_column] = denominator.recip();
            }
            basis[component[local_column + 1] * constrained_dimension + output_column] =
                -(local_column as f64 + 1.0) / denominator;
            output_column += 1;
        }
    }
    basis
}

fn laplacian(weights: &ValidatedSpatialWeights, dimension: usize) -> Vec<f64> {
    let mut result = vec![0.0; dimension * dimension];
    for edge in &weights.weights {
        result[edge.source_index * dimension + edge.target_index] = -edge.weight;
        result[edge.source_index * dimension + edge.source_index] += edge.weight;
    }
    result
}

fn project(
    matrix: &[f64],
    dimension: usize,
    basis: &[f64],
    projected_dimension: usize,
) -> Vec<f64> {
    let mut transformed = vec![0.0; dimension * projected_dimension];
    for row in 0..dimension {
        for column in 0..projected_dimension {
            transformed[row * projected_dimension + column] = (0..dimension)
                .map(|inner| {
                    matrix[row * dimension + inner] * basis[inner * projected_dimension + column]
                })
                .sum();
        }
    }
    let mut projected = vec![0.0; projected_dimension * projected_dimension];
    for row in 0..projected_dimension {
        for column in 0..=row {
            let value = (0..dimension)
                .map(|inner| {
                    basis[inner * projected_dimension + row]
                        * transformed[inner * projected_dimension + column]
                })
                .sum();
            projected[row * projected_dimension + column] = value;
            projected[column * projected_dimension + row] = value;
        }
    }
    projected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate_spatial_weights, SpatialEdge, SpatialWeightsPolicy};

    #[test]
    fn ring_transform_is_sum_zero_and_whitens_constrained_precision() {
        let weights = ring_weights();
        let plan = build_icar_plan(&weights).unwrap();
        assert_eq!(plan.rank_deficiency, 1);
        assert_eq!(plan.constrained_dimension, 5);
        assert!(plan.typical_marginal_variance > 0.0);
        assert!((plan.scaled_typical_marginal_variance - 1.0).abs() <= 1e-12);
        for column in 0..plan.constrained_dimension {
            let sum = (0..plan.dimension)
                .map(|row| plan.transform[row * plan.constrained_dimension + column])
                .sum::<f64>();
            assert!(sum.abs() <= 1e-12);
        }
        let precision = laplacian(&weights, plan.dimension);
        let whitened = project(
            &precision,
            plan.dimension,
            &plan.transform,
            plan.constrained_dimension,
        );
        for row in 0..plan.constrained_dimension {
            for column in 0..plan.constrained_dimension {
                let expected = if row == column { 1.0 } else { 0.0 };
                assert!(
                    (whitened[row * plan.constrained_dimension + column] - expected).abs() <= 1e-12
                );
            }
        }
    }

    fn ring_weights() -> ValidatedSpatialWeights {
        let regions = (0..6).map(|index| format!("r{index}")).collect();
        let mut edges = Vec::new();
        for index in 0..6 {
            edges.push(edge(index, (index + 5) % 6));
            edges.push(edge(index, (index + 1) % 6));
        }
        validate_spatial_weights(
            regions,
            edges,
            SpatialWeightsPolicy {
                symmetry: SymmetryPolicy::Required,
                diagonal: DiagonalPolicy::Zero,
                normalization: NormalizationPolicy::Preserve,
            },
        )
        .unwrap()
    }

    fn edge(source: usize, target: usize) -> SpatialEdge {
        SpatialEdge {
            source_region: format!("r{source}"),
            target_region: format!("r{target}"),
            weight: 1.0,
        }
    }
}
