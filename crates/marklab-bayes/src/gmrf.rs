use std::collections::BTreeSet;

use thiserror::Error;

use crate::linalg;

#[derive(Clone, Debug)]
pub struct GmrfConstraint {
    pub name: String,
    pub coefficients: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct GmrfSpec {
    pub dimension: usize,
    pub precision: Vec<f64>,
    pub field: Vec<f64>,
    pub constraints: Vec<GmrfConstraint>,
    pub constraint_tolerance: f64,
}

#[derive(Clone, Debug)]
pub struct GmrfDensityResult {
    pub log_density: f64,
    pub log_determinant: f64,
    pub quadratic: f64,
    pub rank_deficiency: usize,
    pub constrained_dimension: usize,
    pub constraint_names: Vec<String>,
}

#[derive(Debug, Error)]
pub enum GmrfDensityError {
    #[error("invalid GMRF input: {0}")]
    InvalidInput(String),
    #[error("GMRF numerical failure: {0}")]
    Numerical(String),
}

pub fn gmrf_log_density(spec: GmrfSpec) -> Result<GmrfDensityResult, GmrfDensityError> {
    validate_spec(&spec)?;
    let row_basis = constraint_row_basis(&spec)?;
    let null_basis = null_space_basis(spec.dimension, &row_basis, spec.constraint_tolerance)?;
    let constrained_dimension = null_basis.len();
    let projected = projected_precision(&spec.precision, spec.dimension, &null_basis);
    let lower = linalg::cholesky(&projected, constrained_dimension).ok_or_else(|| {
        GmrfDensityError::InvalidInput(
            "precision is not positive definite on the constrained subspace".into(),
        )
    })?;
    let log_determinant = 2.0
        * (0..constrained_dimension)
            .map(|index| lower[index * constrained_dimension + index].ln())
            .sum::<f64>();
    let quadratic = quadratic(&spec.precision, &spec.field, spec.dimension);
    let log_density = 0.5 * log_determinant
        - 0.5 * quadratic
        - 0.5 * constrained_dimension as f64 * (2.0 * std::f64::consts::PI).ln();
    if !log_density.is_finite() || !log_determinant.is_finite() || !quadratic.is_finite() {
        return Err(GmrfDensityError::Numerical(
            "density calculation produced a non-finite value".into(),
        ));
    }
    Ok(GmrfDensityResult {
        log_density,
        log_determinant,
        quadratic,
        rank_deficiency: row_basis.len(),
        constrained_dimension,
        constraint_names: spec
            .constraints
            .iter()
            .map(|constraint| constraint.name.clone())
            .collect(),
    })
}

fn validate_spec(spec: &GmrfSpec) -> Result<(), GmrfDensityError> {
    if spec.dimension < 2 || spec.dimension > 256 {
        return Err(GmrfDensityError::InvalidInput(
            "dimension must be between 2 and 256".into(),
        ));
    }
    if spec.precision.len() != spec.dimension * spec.dimension || spec.field.len() != spec.dimension
    {
        return Err(GmrfDensityError::InvalidInput(
            "precision and field dimensions do not match".into(),
        ));
    }
    if spec.constraints.len() >= spec.dimension {
        return Err(GmrfDensityError::InvalidInput(
            "constraint count must be less than dimension".into(),
        ));
    }
    if !spec.constraint_tolerance.is_finite() || spec.constraint_tolerance <= 0.0 {
        return Err(GmrfDensityError::InvalidInput(
            "constraint tolerance must be finite and positive".into(),
        ));
    }
    if spec.precision.iter().any(|value| !value.is_finite())
        || spec.field.iter().any(|value| !value.is_finite())
    {
        return Err(GmrfDensityError::InvalidInput(
            "precision and field values must be finite".into(),
        ));
    }
    for row in 0..spec.dimension {
        for column in 0..row {
            if spec.precision[row * spec.dimension + column]
                != spec.precision[column * spec.dimension + row]
            {
                return Err(GmrfDensityError::InvalidInput(
                    "precision must be exactly symmetric".into(),
                ));
            }
        }
    }
    Ok(())
}

fn constraint_row_basis(spec: &GmrfSpec) -> Result<Vec<Vec<f64>>, GmrfDensityError> {
    let mut names = BTreeSet::new();
    let mut basis = Vec::with_capacity(spec.constraints.len());
    for constraint in &spec.constraints {
        if constraint.name.is_empty()
            || constraint.name.trim() != constraint.name
            || !names.insert(constraint.name.as_str())
            || constraint.coefficients.len() != spec.dimension
            || constraint
                .coefficients
                .iter()
                .any(|value| !value.is_finite())
        {
            return Err(GmrfDensityError::InvalidInput(
                "constraints require unique exact names and finite dimension-matched rows".into(),
            ));
        }
        let original_norm = norm(&constraint.coefficients);
        if original_norm == 0.0 {
            return Err(GmrfDensityError::InvalidInput(format!(
                "constraint {} has no nonzero coefficient",
                constraint.name
            )));
        }
        let residual = dot(&constraint.coefficients, &spec.field);
        if residual.abs() > spec.constraint_tolerance {
            return Err(GmrfDensityError::InvalidInput(format!(
                "field violates constraint {} by {residual}",
                constraint.name
            )));
        }
        let mut candidate = constraint.coefficients.clone();
        orthogonalize(&mut candidate, &basis);
        let candidate_norm = norm(&candidate);
        if candidate_norm <= spec.constraint_tolerance * original_norm.max(1.0) {
            return Err(GmrfDensityError::InvalidInput(format!(
                "constraint {} is linearly dependent under the declared tolerance",
                constraint.name
            )));
        }
        scale(&mut candidate, candidate_norm.recip());
        basis.push(candidate);
    }
    Ok(basis)
}

fn null_space_basis(
    dimension: usize,
    row_basis: &[Vec<f64>],
    tolerance: f64,
) -> Result<Vec<Vec<f64>>, GmrfDensityError> {
    let expected = dimension - row_basis.len();
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(expected);
    for axis in 0..dimension {
        let mut candidate = vec![0.0; dimension];
        candidate[axis] = 1.0;
        orthogonalize(&mut candidate, row_basis);
        orthogonalize(&mut candidate, &basis);
        let candidate_norm = norm(&candidate);
        if candidate_norm > tolerance {
            scale(&mut candidate, candidate_norm.recip());
            basis.push(candidate);
            if basis.len() == expected {
                return Ok(basis);
            }
        }
    }
    Err(GmrfDensityError::Numerical(
        "could not construct the declared constrained subspace".into(),
    ))
}

fn orthogonalize(candidate: &mut [f64], basis: &[Vec<f64>]) {
    for _ in 0..2 {
        for vector in basis {
            let projection = dot(candidate, vector);
            for (value, basis_value) in candidate.iter_mut().zip(vector) {
                *value -= projection * basis_value;
            }
        }
    }
}

fn projected_precision(precision: &[f64], dimension: usize, basis: &[Vec<f64>]) -> Vec<f64> {
    let projected_dimension = basis.len();
    let transformed = basis
        .iter()
        .map(|vector| {
            (0..dimension)
                .map(|row| {
                    (0..dimension)
                        .map(|column| precision[row * dimension + column] * vector[column])
                        .sum::<f64>()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut projected = vec![0.0; projected_dimension * projected_dimension];
    for row in 0..projected_dimension {
        for column in 0..=row {
            let value = dot(&basis[row], &transformed[column]);
            projected[row * projected_dimension + column] = value;
            projected[column * projected_dimension + row] = value;
        }
    }
    projected
}

fn quadratic(precision: &[f64], field: &[f64], dimension: usize) -> f64 {
    (0..dimension)
        .map(|row| {
            field[row]
                * (0..dimension)
                    .map(|column| precision[row * dimension + column] * field[column])
                    .sum::<f64>()
        })
        .sum()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn norm(vector: &[f64]) -> f64 {
    dot(vector, vector).sqrt()
}

fn scale(vector: &mut [f64], scale: f64) {
    for value in vector {
        *value *= scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_diagonal_matches_hand_oracle() {
        let result = gmrf_log_density(GmrfSpec {
            dimension: 3,
            precision: vec![2.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 4.0],
            field: vec![0.5, -0.5, 0.25],
            constraints: vec![GmrfConstraint {
                name: "sum_ab".into(),
                coefficients: vec![1.0, 1.0, 0.0],
            }],
            constraint_tolerance: 1e-12,
        })
        .unwrap();
        assert!((result.log_determinant - 10.0_f64.ln()).abs() <= 1e-12);
        assert!((result.quadratic - 1.5).abs() <= 1e-12);
        assert!((result.log_density + 1.4365845199123224).abs() <= 1e-12);
    }

    #[test]
    fn rejects_dependent_constraints_and_nonpositive_subspace() {
        let dependent = gmrf_log_density(GmrfSpec {
            dimension: 3,
            precision: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            field: vec![0.0; 3],
            constraints: vec![
                GmrfConstraint {
                    name: "one".into(),
                    coefficients: vec![1.0, 1.0, 0.0],
                },
                GmrfConstraint {
                    name: "two".into(),
                    coefficients: vec![2.0, 2.0, 0.0],
                },
            ],
            constraint_tolerance: 1e-12,
        })
        .unwrap_err();
        assert!(dependent.to_string().contains("linearly dependent"));

        let indefinite = gmrf_log_density(GmrfSpec {
            dimension: 2,
            precision: vec![1.0, 0.0, 0.0, -1.0],
            field: vec![0.0; 2],
            constraints: Vec::new(),
            constraint_tolerance: 1e-12,
        })
        .unwrap_err();
        assert!(indefinite
            .to_string()
            .contains("not positive definite on the constrained subspace"));
    }

    #[test]
    fn rejects_nonsymmetric_precision_and_constraint_violation() {
        let nonsymmetric = gmrf_log_density(GmrfSpec {
            dimension: 2,
            precision: vec![2.0, 1.0, 0.0, 2.0],
            field: vec![0.0; 2],
            constraints: Vec::new(),
            constraint_tolerance: 1e-12,
        })
        .unwrap_err();
        assert!(nonsymmetric.to_string().contains("exactly symmetric"));

        let violation = gmrf_log_density(GmrfSpec {
            dimension: 2,
            precision: vec![2.0, 0.0, 0.0, 2.0],
            field: vec![0.1, 0.0],
            constraints: vec![GmrfConstraint {
                name: "first_zero".into(),
                coefficients: vec![1.0, 0.0],
            }],
            constraint_tolerance: 1e-12,
        })
        .unwrap_err();
        assert!(violation
            .to_string()
            .contains("violates constraint first_zero"));
    }
}
