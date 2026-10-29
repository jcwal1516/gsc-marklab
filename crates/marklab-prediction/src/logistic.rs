//! L2-penalised logistic regression on standardised features.
//!
//! A fit minimises `½‖β‖² + C Σᵢ sᵢ [log(1 + exp(mᵢ)) − yᵢ mᵢ]` with margins `mᵢ = x̃ᵢ·β + b`.
//! The features `x̃` are standardised with the training mean and population standard deviation,
//! `sᵢ` is the row's class weight and the intercept `b` is not penalised. This is scikit-learn's
//! `StandardScaler` followed by `LogisticRegression(C = C)` with its default L2 penalty.
//!
//! With fewer rows than features the minimiser lies in the row space of `x̃`, so the problem is
//! solved there: the Householder factorisation `x̃ᵀ = Q R` gives margins `Rᵀγ + b` with `β = Qγ`
//! and `‖β‖ = ‖γ‖`. Newton's method with a dense Cholesky solve then works in at most
//! `min(rows, features) + 1` dimensions.

use marklab_numerics::{
    dense::{cholesky, cholesky_solve},
    sigmoid,
    summation::{neumaier_sum, NeumaierSum},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Largest `min(rows, features)`: the dimension of each Newton system.
pub const MAXIMUM_LOGISTIC_DIMENSION: usize = 4096;
/// Largest number of feature values, `rows × features`, in one training set.
pub const MAXIMUM_LOGISTIC_VALUES: usize = 100_000_000;
const MAXIMUM_NEWTON_ITERATIONS: usize = 200;

/// Weight of each training row in the loss.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClassWeight {
    /// Every row has weight one.
    Uniform,
    /// Rows of a class with `n_c` of the `n` rows have weight `n / (2 n_c)`, scikit-learn's
    /// `class_weight = "balanced"`.
    Balanced,
}

/// Failure of a logistic regression fit or cross-validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LogisticError {
    #[error("invalid logistic regression: {0}")]
    Invalid(String),
    #[error("logistic regression resource limit exceeded: {0}")]
    Resource(String),
    #[error("logistic regression numerical failure: {0}")]
    Numerical(String),
}

/// Standardised training data, prepared once and fitted for any number of `C` values.
pub struct LogisticProblem {
    columns: usize,
    mean: Vec<f64>,
    scale: Vec<f64>,
    labels: Vec<bool>,
    weights: Vec<f64>,
    /// Row-major `rows × dimension` design whose products with the parameters are the margins.
    design: Vec<f64>,
    dimension: usize,
    reduction: Option<RowSpace>,
}

/// Householder reflectors of `x̃ᵀ = Q R`, stored as in LAPACK's `geqrf`: column `k` of `x̃ᵀ`
/// (row `k` of `packed`) holds the reflector below position `k`; its first entry is `heads[k]`.
struct RowSpace {
    packed: Vec<f64>,
    heads: Vec<f64>,
    norms: Vec<f64>,
}

/// A fitted model: the training standardisation, the coefficients on standardised features and
/// the intercept.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LogisticModel {
    mean: Vec<f64>,
    scale: Vec<f64>,
    coefficients: Vec<f64>,
    intercept: f64,
    #[serde(skip)]
    newton_iterations: usize,
}

impl LogisticProblem {
    /// Standardise `features` (row-major, `labels.len()` rows of `columns` values) and reduce
    /// them to their row space when there are fewer rows than columns.
    ///
    /// A column whose training values are all equal is centred on that value and left unscaled,
    /// as in scikit-learn, so it receives a zero coefficient.
    pub fn new(
        features: &[f64],
        columns: usize,
        labels: &[bool],
        class_weight: ClassWeight,
    ) -> Result<Self, LogisticError> {
        let rows = labels.len();
        if columns == 0 || rows.checked_mul(columns) != Some(features.len()) {
            return Err(LogisticError::Invalid(format!(
                "{} feature values do not form {rows} rows of {columns} columns",
                features.len()
            )));
        }
        if features.len() > MAXIMUM_LOGISTIC_VALUES {
            return Err(LogisticError::Resource(format!(
                "{rows} rows × {columns} features exceed {MAXIMUM_LOGISTIC_VALUES} values"
            )));
        }
        if rows.min(columns) > MAXIMUM_LOGISTIC_DIMENSION {
            return Err(LogisticError::Resource(format!(
                "{rows} rows and {columns} features both exceed {MAXIMUM_LOGISTIC_DIMENSION}"
            )));
        }
        if !features.iter().all(|value| value.is_finite()) {
            return Err(LogisticError::Invalid(
                "feature values must be finite".to_owned(),
            ));
        }
        let positives = labels.iter().filter(|&&label| label).count();
        if positives == 0 || positives == rows {
            return Err(LogisticError::Invalid(
                "training rows must include both classes".to_owned(),
            ));
        }
        let weights = labels
            .iter()
            .map(|&label| match class_weight {
                ClassWeight::Uniform => 1.0,
                ClassWeight::Balanced => {
                    let class_rows = if label { positives } else { rows - positives };
                    rows as f64 / (2.0 * class_rows as f64)
                }
            })
            .collect();
        let (mean, scale) = standardisation(features, rows, columns)?;
        let mut standardised = features.to_vec();
        for row in standardised.chunks_exact_mut(columns) {
            for ((value, mean), scale) in row.iter_mut().zip(&mean).zip(&scale) {
                *value = (*value - mean) / scale;
            }
        }
        let (design, dimension, reduction) = if rows < columns {
            let (design, reduction) = row_space(standardised, rows, columns);
            (design, rows, Some(reduction))
        } else {
            (standardised, columns, None)
        };
        Ok(Self {
            columns,
            mean,
            scale,
            labels: labels.to_vec(),
            weights,
            design,
            dimension,
            reduction,
        })
    }

    /// Fit with inverse penalty strength `c`.
    pub fn fit(&self, c: f64) -> Result<LogisticModel, LogisticError> {
        Ok(self.fit_path(&[c])?.remove(0))
    }

    /// Fit each of the strictly increasing `c_values`, starting every fit after the first from
    /// the previous solution. The minimiser for each `C` is unique, so the result does not
    /// depend on the starting point beyond the convergence tolerance.
    pub fn fit_path(&self, c_values: &[f64]) -> Result<Vec<LogisticModel>, LogisticError> {
        if !c_values.iter().all(|c| c.is_finite() && *c > 0.0)
            || !c_values.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(LogisticError::Invalid(
                "C values must be finite, positive and strictly increasing".to_owned(),
            ));
        }
        let mut start = vec![0.0; self.dimension + 1];
        let mut models = Vec::with_capacity(c_values.len());
        for &c in c_values {
            let (parameters, newton_iterations) = self.newton(c, start)?;
            start = parameters.clone();
            models.push(self.model(parameters, newton_iterations));
        }
        Ok(models)
    }

    fn model(&self, parameters: Vec<f64>, newton_iterations: usize) -> LogisticModel {
        let intercept = parameters[self.dimension];
        let mut coefficients = parameters;
        coefficients.truncate(self.dimension);
        if let Some(reduction) = &self.reduction {
            coefficients = reduction.expand(&coefficients, self.columns);
        }
        LogisticModel {
            mean: self.mean.clone(),
            scale: self.scale.clone(),
            coefficients,
            intercept,
            newton_iterations,
        }
    }

    fn rows(&self) -> usize {
        self.labels.len()
    }

    /// Damped Newton iterations from `start` (the coefficients of the design followed by the
    /// intercept); returns the minimiser in the same layout and the number of iterations.
    fn newton(&self, c: f64, start: Vec<f64>) -> Result<(Vec<f64>, usize), LogisticError> {
        let size = self.dimension + 1;
        let mut parameters = start;
        let mut margins = self.margins(&parameters);
        let mut objective = self.objective(c, &parameters, &margins);
        let mut previous_decrement = f64::INFINITY;
        for iteration in 1..=MAXIMUM_NEWTON_ITERATIONS {
            let (gradient, hessian) = self.gradient_and_hessian(c, &parameters, &margins);
            let lower = cholesky(&hessian, size).ok_or_else(|| {
                LogisticError::Numerical(format!(
                    "the Newton system at C = {c} is not positive definite"
                ))
            })?;
            let step: Vec<f64> = cholesky_solve(&lower, size, &gradient)
                .into_iter()
                .map(|value| -value)
                .collect();
            // Newton decrement squared: twice the predicted decrease of the objective.
            let decrement: f64 = -gradient.iter().zip(&step).map(|(g, s)| g * s).sum::<f64>();
            if !decrement.is_finite() {
                return Err(LogisticError::Numerical(format!(
                    "the Newton step at C = {c} is not finite"
                )));
            }
            let reference = objective.max(1.0);
            let step_margins = self.margins(&step);
            let trial = |length: f64| {
                let parameters: Vec<f64> = parameters
                    .iter()
                    .zip(&step)
                    .map(|(p, s)| p + length * s)
                    .collect();
                let margins: Vec<f64> = margins
                    .iter()
                    .zip(&step_margins)
                    .map(|(m, s)| m + length * s)
                    .collect();
                let objective = self.objective(c, &parameters, &margins);
                (parameters, margins, objective)
            };
            if decrement <= 1e-10 * reference {
                // Close to the minimum, full steps converge quadratically. Stop when the
                // predicted decrease is negligible or rounding stops it from shrinking.
                let (next, next_margins, next_objective) = trial(1.0);
                if next_objective > objective + 1e-12 * reference {
                    return Ok((parameters, iteration));
                }
                (parameters, margins, objective) = (next, next_margins, next_objective);
                if decrement <= 1e-14 * reference || decrement >= previous_decrement {
                    return Ok((parameters, iteration));
                }
                previous_decrement = decrement;
                continue;
            }
            previous_decrement = f64::INFINITY;
            let mut length = 1.0;
            loop {
                let (next, next_margins, next_objective) = trial(length);
                if next_objective <= objective - 1e-4 * length * decrement {
                    (parameters, margins, objective) = (next, next_margins, next_objective);
                    break;
                }
                length *= 0.5;
                if length < 1e-10 {
                    return Err(LogisticError::Numerical(format!(
                        "the line search at C = {c} found no decrease"
                    )));
                }
            }
        }
        Err(LogisticError::Numerical(format!(
            "Newton's method did not converge at C = {c} in {MAXIMUM_NEWTON_ITERATIONS} iterations"
        )))
    }

    fn margins(&self, parameters: &[f64]) -> Vec<f64> {
        let intercept = parameters[self.dimension];
        self.design
            .chunks_exact(self.dimension)
            .map(|row| intercept + row.iter().zip(parameters).map(|(z, p)| z * p).sum::<f64>())
            .collect()
    }

    fn objective(&self, c: f64, parameters: &[f64], margins: &[f64]) -> f64 {
        let mut loss = NeumaierSum::new();
        for ((&margin, &label), &weight) in margins.iter().zip(&self.labels).zip(&self.weights) {
            // log(1 + exp(m)) − y m, written as softplus(±m) to avoid overflow.
            loss.add(weight * softplus(if label { -margin } else { margin }));
        }
        let penalty: f64 = parameters[..self.dimension].iter().map(|p| p * p).sum();
        0.5 * penalty + c * loss.total()
    }

    /// Gradient and the full (row-major) Hessian of the objective, intercept last.
    fn gradient_and_hessian(
        &self,
        c: f64,
        parameters: &[f64],
        margins: &[f64],
    ) -> (Vec<f64>, Vec<f64>) {
        let dimension = self.dimension;
        let size = dimension + 1;
        let mut gradient = parameters.to_vec();
        gradient[dimension] = 0.0;
        let mut hessian = vec![0.0; size * size];
        let rows = self.design.chunks_exact(dimension);
        for (((row, &margin), &label), &weight) in
            rows.zip(margins).zip(&self.labels).zip(&self.weights)
        {
            let probability = sigmoid(margin);
            let residual = c * weight * (probability - f64::from(u8::from(label)));
            let curvature = c * weight * probability * (1.0 - probability);
            for (g, z) in gradient.iter_mut().zip(row) {
                *g += residual * z;
            }
            gradient[dimension] += residual;
            for (j, &zj) in row.iter().enumerate() {
                let scaled = curvature * zj;
                if scaled != 0.0 {
                    let target = &mut hessian[j * size..=j * size + j];
                    for (h, z) in target.iter_mut().zip(row) {
                        *h += scaled * z;
                    }
                }
            }
            let intercept_row = &mut hessian[dimension * size..dimension * size + dimension];
            for (h, z) in intercept_row.iter_mut().zip(row) {
                *h += curvature * z;
            }
            hessian[dimension * size + dimension] += curvature;
        }
        for j in 0..dimension {
            hessian[j * size + j] += 1.0;
        }
        // The intercept is unpenalised; this keeps its Newton equation solvable when every row
        // is fitted almost exactly, without changing the objective being minimised.
        hessian[dimension * size + dimension] += 1e-12 * c * self.rows() as f64;
        (gradient, hessian)
    }
}

impl RowSpace {
    /// `β = Q [γ; 0]`, applying the reflectors in reverse order.
    fn expand(&self, reduced: &[f64], columns: usize) -> Vec<f64> {
        let mut expanded = vec![0.0; columns];
        expanded[..reduced.len()].copy_from_slice(reduced);
        for k in (0..reduced.len()).rev() {
            if self.norms[k] == 0.0 {
                continue;
            }
            let tail = &self.packed[k * columns + k + 1..(k + 1) * columns];
            let product = self.heads[k] * expanded[k]
                + tail
                    .iter()
                    .zip(&expanded[k + 1..])
                    .map(|(v, x)| v * x)
                    .sum::<f64>();
            let factor = 2.0 * product / self.norms[k];
            expanded[k] -= factor * self.heads[k];
            for (x, v) in expanded[k + 1..].iter_mut().zip(tail) {
                *x -= factor * v;
            }
        }
        expanded
    }
}

impl LogisticModel {
    /// Predicted probability of the positive class for one row of raw (unstandardised)
    /// features.
    pub fn probability(&self, row: &[f64]) -> Result<f64, LogisticError> {
        self.margin(row).map(sigmoid)
    }

    /// Log-odds of the positive class, `x̃·β + b`, for one row of raw features.
    pub fn margin(&self, row: &[f64]) -> Result<f64, LogisticError> {
        if row.len() != self.coefficients.len() {
            return Err(LogisticError::Invalid(format!(
                "a row has {} features; the model was fitted on {}",
                row.len(),
                self.coefficients.len()
            )));
        }
        if !row.iter().all(|value| value.is_finite()) {
            return Err(LogisticError::Invalid(
                "feature values must be finite".to_owned(),
            ));
        }
        Ok(self.intercept
            + row
                .iter()
                .zip(&self.mean)
                .zip(&self.scale)
                .zip(&self.coefficients)
                .map(|(((x, mean), scale), beta)| (x - mean) / scale * beta)
                .sum::<f64>())
    }

    /// Number of features the model was fitted on.
    pub fn columns(&self) -> usize {
        self.coefficients.len()
    }

    /// Reject a deserialised model whose parts are inconsistent or not finite.
    pub(crate) fn check(&self) -> Result<(), LogisticError> {
        let columns = self.coefficients.len();
        if columns == 0 || self.mean.len() != columns || self.scale.len() != columns {
            return Err(LogisticError::Invalid(
                "a model's mean, scale and coefficients must have one value per feature".to_owned(),
            ));
        }
        let finite = self
            .mean
            .iter()
            .chain(&self.coefficients)
            .all(|value| value.is_finite())
            && self.intercept.is_finite()
            && self
                .scale
                .iter()
                .all(|scale| scale.is_finite() && *scale > 0.0);
        if !finite {
            return Err(LogisticError::Invalid(
                "a model's values must be finite and its scales positive".to_owned(),
            ));
        }
        Ok(())
    }

    /// Newton iterations the fit took.
    pub fn newton_iterations(&self) -> usize {
        self.newton_iterations
    }
}

fn softplus(value: f64) -> f64 {
    value.max(0.0) + (-value.abs()).exp().ln_1p()
}

fn standardisation(
    features: &[f64],
    rows: usize,
    columns: usize,
) -> Result<(Vec<f64>, Vec<f64>), LogisticError> {
    let mut mean = Vec::with_capacity(columns);
    let mut scale = Vec::with_capacity(columns);
    for column in 0..columns {
        let values = || (0..rows).map(|row| features[row * columns + column]);
        let first = features[column];
        if values().all(|value| value == first) {
            mean.push(first);
            scale.push(1.0);
            continue;
        }
        let centre = neumaier_sum(values()) / rows as f64;
        let variance = neumaier_sum(values().map(|value| (value - centre).powi(2))) / rows as f64;
        let deviation = variance.sqrt();
        if !(deviation.is_finite() && deviation > 0.0) {
            return Err(LogisticError::Numerical(format!(
                "feature column {column} cannot be standardised"
            )));
        }
        mean.push(centre);
        scale.push(deviation);
    }
    Ok((mean, scale))
}

/// Householder QR of `x̃ᵀ` (`columns × rows`, stored as the row-major `x̃`). Returns the
/// row-major `rows × rows` design `Rᵀ` and the reflectors.
fn row_space(mut packed: Vec<f64>, rows: usize, columns: usize) -> (Vec<f64>, RowSpace) {
    let mut heads = vec![0.0; rows];
    let mut norms = vec![0.0; rows];
    for k in 0..rows {
        let (done, rest) = packed.split_at_mut((k + 1) * columns);
        let column = &mut done[k * columns..];
        let length = column[k..].iter().map(|v| v * v).sum::<f64>().sqrt();
        if length == 0.0 {
            continue;
        }
        let diagonal = -length.copysign(column[k]);
        heads[k] = column[k] - diagonal;
        norms[k] = heads[k] * heads[k] + column[k + 1..].iter().map(|v| v * v).sum::<f64>();
        column[k] = diagonal;
        let tail = &column[k + 1..];
        for other in rest.chunks_exact_mut(columns) {
            let product = heads[k] * other[k]
                + tail
                    .iter()
                    .zip(&other[k + 1..])
                    .map(|(v, x)| v * x)
                    .sum::<f64>();
            let factor = 2.0 * product / norms[k];
            other[k] -= factor * heads[k];
            for (x, v) in other[k + 1..].iter_mut().zip(tail) {
                *x -= factor * v;
            }
        }
    }
    // Rᵀ: row i holds R[k][i] for k ≤ i, which is entry k of packed column i.
    let mut design = vec![0.0; rows * rows];
    for i in 0..rows {
        design[i * rows..=i * rows + i].copy_from_slice(&packed[i * columns..=i * columns + i]);
    }
    (
        design,
        RowSpace {
            packed,
            heads,
            norms,
        },
    )
}
