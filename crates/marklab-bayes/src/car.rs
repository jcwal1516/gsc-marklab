use std::collections::BTreeMap;

use thiserror::Error;

use crate::{linalg, DiagonalPolicy, NormalizationPolicy, SymmetryPolicy, ValidatedSpatialWeights};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarMode {
    Proper,
    Intrinsic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IslandPolicy {
    Reject,
    Exclude,
}

#[derive(Clone, Debug)]
pub struct RegionFieldValue {
    pub region_id: String,
    pub value: f64,
}

#[derive(Clone, Debug)]
pub struct CarSpec {
    pub mode: CarMode,
    pub tau: f64,
    pub rho: f64,
    pub constraint_tolerance: f64,
    pub island_policy: IslandPolicy,
    pub weights: ValidatedSpatialWeights,
    pub field: Vec<RegionFieldValue>,
}

#[derive(Clone, Debug)]
pub struct CarDensityResult {
    pub log_density: f64,
    pub rank_deficiency: usize,
    pub constraints: Vec<Vec<usize>>,
    pub excluded_islands: Vec<usize>,
}

#[derive(Debug, Error)]
pub enum CarDensityError {
    #[error("invalid CAR input: {0}")]
    InvalidInput(String),
    #[error("CAR numerical failure: {0}")]
    Numerical(String),
}

pub fn car_density(spec: CarSpec) -> Result<CarDensityResult, CarDensityError> {
    if spec.weights.policy.symmetry != SymmetryPolicy::Required
        || spec.weights.policy.diagonal != DiagonalPolicy::Zero
        || spec.weights.policy.normalization != NormalizationPolicy::Preserve
    {
        return Err(CarDensityError::InvalidInput(
            "CAR requires preserved symmetric zero-diagonal weights".into(),
        ));
    }
    if spec.weights.region_ids.len() < 2 || spec.weights.region_ids.len() > 512 {
        return Err(CarDensityError::InvalidInput(
            "CAR region count must be between 2 and 512".into(),
        ));
    }
    if spec.weights.weights.len() > 200_000 {
        return Err(CarDensityError::InvalidInput(
            "CAR accepts at most 200000 edges".into(),
        ));
    }
    if !spec.tau.is_finite() || spec.tau <= 0.0 {
        return Err(CarDensityError::InvalidInput(
            "tau must be finite and positive".into(),
        ));
    }
    if !spec.rho.is_finite()
        || !spec.constraint_tolerance.is_finite()
        || spec.constraint_tolerance <= 0.0
    {
        return Err(CarDensityError::InvalidInput(
            "rho must be finite and constraint tolerance positive".into(),
        ));
    }
    let field = bind_field(&spec.weights.region_ids, &spec.field)?;
    let dimension = spec.weights.region_ids.len();
    let mut weight_matrix = vec![0.0; dimension * dimension];
    let mut degrees = vec![0.0; dimension];
    for edge in &spec.weights.weights {
        weight_matrix[edge.source_index * dimension + edge.target_index] = edge.weight;
        degrees[edge.source_index] += edge.weight;
    }
    match spec.mode {
        CarMode::Proper => proper_density(&spec, &field, &weight_matrix, &degrees),
        CarMode::Intrinsic => intrinsic_density(&spec, &field, &weight_matrix, &degrees),
    }
}

fn bind_field(
    region_ids: &[String],
    field: &[RegionFieldValue],
) -> Result<Vec<f64>, CarDensityError> {
    if field.len() != region_ids.len() {
        return Err(CarDensityError::InvalidInput(
            "field must contain exactly one row per region".into(),
        ));
    }
    let mut values = BTreeMap::new();
    for row in field {
        if !row.value.is_finite() || values.insert(row.region_id.as_str(), row.value).is_some() {
            return Err(CarDensityError::InvalidInput(
                "field region IDs must be unique with finite values".into(),
            ));
        }
    }
    region_ids
        .iter()
        .map(|id| {
            values
                .remove(id.as_str())
                .ok_or_else(|| CarDensityError::InvalidInput(format!("missing field region {id}")))
        })
        .collect()
}

fn proper_density(
    spec: &CarSpec,
    field: &[f64],
    weights: &[f64],
    degrees: &[f64],
) -> Result<CarDensityResult, CarDensityError> {
    if !spec.weights.islands.is_empty() {
        return Err(CarDensityError::InvalidInput(
            "proper CAR version one rejects islands".into(),
        ));
    }
    let dimension = field.len();
    let mut precision = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            precision[row * dimension + column] = if row == column {
                spec.tau * degrees[row]
            } else {
                -spec.tau * spec.rho * weights[row * dimension + column]
            };
        }
    }
    let lower = linalg::cholesky(&precision, dimension).ok_or_else(|| {
        CarDensityError::InvalidInput(
            "rho is outside the interval yielding positive-definite proper CAR precision".into(),
        )
    })?;
    let log_density = normalized_density(&precision, &lower, field, dimension);
    Ok(CarDensityResult {
        log_density,
        rank_deficiency: 0,
        constraints: Vec::new(),
        excluded_islands: Vec::new(),
    })
}

fn intrinsic_density(
    spec: &CarSpec,
    field: &[f64],
    weights: &[f64],
    degrees: &[f64],
) -> Result<CarDensityResult, CarDensityError> {
    if !spec.weights.islands.is_empty() && spec.island_policy == IslandPolicy::Reject {
        return Err(CarDensityError::InvalidInput(
            "intrinsic CAR has islands under reject policy".into(),
        ));
    }
    let dimension = field.len();
    let mut precision = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            precision[row * dimension + column] = if row == column {
                spec.tau * degrees[row]
            } else {
                -spec.tau * weights[row * dimension + column]
            };
        }
    }
    let island_set = spec
        .weights
        .islands
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut log_density = 0.0;
    let mut constraints = Vec::new();
    let mut rank_deficiency = 0;
    for component in &spec.weights.components {
        if component.len() == 1 && island_set.contains(&component[0]) {
            continue;
        }
        let sum = component.iter().map(|&index| field[index]).sum::<f64>();
        if sum.abs() > spec.constraint_tolerance {
            return Err(CarDensityError::InvalidInput(format!(
                "component field sum {sum} exceeds constraint tolerance {}",
                spec.constraint_tolerance
            )));
        }
        let reduced_dimension = component.len() - 1;
        let mut reduced = vec![0.0; reduced_dimension * reduced_dimension];
        for row in 0..reduced_dimension {
            for column in 0..reduced_dimension {
                reduced[row * reduced_dimension + column] =
                    precision[component[row] * dimension + component[column]];
            }
        }
        let lower = linalg::cholesky(&reduced, reduced_dimension).ok_or_else(|| {
            CarDensityError::Numerical(
                "precision is not positive definite on the required subspace".into(),
            )
        })?;
        let log_nonzero_determinant = (component.len() as f64).ln()
            + 2.0
                * (0..reduced_dimension)
                    .map(|index| lower[index * reduced_dimension + index].ln())
                    .sum::<f64>();
        let mut quadratic = 0.0;
        for &row in component {
            for &column in component {
                quadratic += field[row] * precision[row * dimension + column] * field[column];
            }
        }
        log_density += 0.5 * log_nonzero_determinant
            - 0.5 * quadratic
            - 0.5 * reduced_dimension as f64 * (2.0 * std::f64::consts::PI).ln();
        constraints.push(component.clone());
        rank_deficiency += 1;
    }
    if !log_density.is_finite() {
        return Err(CarDensityError::Numerical(
            "intrinsic constrained density is non-finite".into(),
        ));
    }
    Ok(CarDensityResult {
        log_density,
        rank_deficiency,
        constraints,
        excluded_islands: spec.weights.islands.clone(),
    })
}

fn normalized_density(precision: &[f64], lower: &[f64], field: &[f64], dimension: usize) -> f64 {
    let log_determinant = 2.0
        * (0..dimension)
            .map(|index| lower[index * dimension + index].ln())
            .sum::<f64>();
    let mut quadratic = 0.0;
    for row in 0..dimension {
        for column in 0..dimension {
            quadratic += field[row] * precision[row * dimension + column] * field[column];
        }
    }
    0.5 * log_determinant
        - 0.5 * quadratic
        - 0.5 * dimension as f64 * (2.0 * std::f64::consts::PI).ln()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        validate_spatial_weights, DiagonalPolicy, NormalizationPolicy, SpatialEdge,
        SpatialWeightsPolicy, SymmetryPolicy,
    };

    #[test]
    fn chain_matches_proper_and_intrinsic_oracles() {
        let weights = chain_weights();
        let field = vec![value("a", 0.2), value("b", -0.1), value("c", -0.1)];
        let proper = car_density(CarSpec {
            mode: CarMode::Proper,
            tau: 1.5,
            rho: 0.2,
            constraint_tolerance: 1e-12,
            island_policy: IslandPolicy::Reject,
            weights: weights.clone(),
            field: field.clone(),
        })
        .unwrap();
        let intrinsic = car_density(CarSpec {
            mode: CarMode::Intrinsic,
            tau: 1.5,
            rho: 0.0,
            constraint_tolerance: 1e-12,
            island_policy: IslandPolicy::Reject,
            weights,
            field,
        })
        .unwrap();
        assert!((proper.log_density + 1.8779553444319261).abs() <= 1e-12);
        assert!((intrinsic.log_density + 0.9506058139671261).abs() <= 1e-12);
    }

    #[test]
    fn proper_rejects_singular_precision() {
        let error = car_density(CarSpec {
            mode: CarMode::Proper,
            tau: 1.5,
            rho: 1.0,
            constraint_tolerance: 1e-12,
            island_policy: IslandPolicy::Reject,
            weights: chain_weights(),
            field: vec![value("a", 0.2), value("b", -0.1), value("c", -0.1)],
        })
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("positive-definite proper CAR precision"));
    }

    #[test]
    fn intrinsic_island_policy_is_explicit() {
        let weights = validate_spatial_weights(
            vec!["a".into(), "b".into(), "c".into(), "d".into()],
            vec![
                edge("a", "b"),
                edge("b", "a"),
                edge("b", "c"),
                edge("c", "b"),
            ],
            SpatialWeightsPolicy {
                symmetry: SymmetryPolicy::Required,
                diagonal: DiagonalPolicy::Zero,
                normalization: NormalizationPolicy::Preserve,
            },
        )
        .unwrap();
        let spec = CarSpec {
            mode: CarMode::Intrinsic,
            tau: 1.5,
            rho: 0.0,
            constraint_tolerance: 1e-12,
            island_policy: IslandPolicy::Reject,
            weights,
            field: vec![
                value("a", 0.2),
                value("b", -0.1),
                value("c", -0.1),
                value("d", 42.0),
            ],
        };
        assert!(car_density(spec.clone())
            .unwrap_err()
            .to_string()
            .contains("islands under reject policy"));

        let result = car_density(CarSpec {
            island_policy: IslandPolicy::Exclude,
            ..spec
        })
        .unwrap();
        assert_eq!(result.excluded_islands, vec![3]);
        assert_eq!(result.constraints, vec![vec![0, 1, 2]]);
        assert_eq!(result.rank_deficiency, 1);
    }

    #[test]
    fn car_rejects_oversized_weight_representation() {
        let mut weights = chain_weights();
        weights.weights = vec![weights.weights[0].clone(); 200_001];
        let error = car_density(CarSpec {
            mode: CarMode::Proper,
            tau: 1.5,
            rho: 0.2,
            constraint_tolerance: 1e-12,
            island_policy: IslandPolicy::Reject,
            weights,
            field: vec![value("a", 0.2), value("b", -0.1), value("c", -0.1)],
        })
        .unwrap_err();
        assert!(error.to_string().contains("at most 200000 edges"));
    }

    fn chain_weights() -> ValidatedSpatialWeights {
        validate_spatial_weights(
            vec!["a".into(), "b".into(), "c".into()],
            vec![
                edge("a", "b"),
                edge("b", "a"),
                edge("b", "c"),
                edge("c", "b"),
            ],
            SpatialWeightsPolicy {
                symmetry: SymmetryPolicy::Required,
                diagonal: DiagonalPolicy::Zero,
                normalization: NormalizationPolicy::Preserve,
            },
        )
        .unwrap()
    }

    fn edge(source: &str, target: &str) -> SpatialEdge {
        SpatialEdge {
            source_region: source.into(),
            target_region: target.into(),
            weight: 1.0,
        }
    }

    fn value(region_id: &str, value: f64) -> RegionFieldValue {
        RegionFieldValue {
            region_id: region_id.into(),
            value,
        }
    }
}
