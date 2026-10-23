//! Numerical OLS fitting shared by cohort designs. Design construction and randomization stay with callers.

use crate::CohortInferenceError;

pub(crate) struct OlsFit {
    pub(crate) fitted: Vec<f64>,
    pub(crate) target_coefficient: f64,
    pub(crate) target_standard_error: f64,
    pub(crate) target_statistic: f64,
    pub(crate) residual_degrees_of_freedom: usize,
}

/// Preserve each workflow's diagnostic vocabulary without duplicating the fitter.
#[derive(Clone, Copy)]
pub(crate) enum OlsContext {
    Covariate,
    Repeated,
}

impl OlsContext {
    /// Fit a design, accounting for fixed effects already absorbed by the caller.
    pub(crate) fn fit(
        self,
        design: &[Vec<f64>],
        outcome: &[f64],
        target_index: Option<usize>,
        absorbed: usize,
    ) -> Result<OlsFit, CohortInferenceError> {
        let (model, no_degrees_of_freedom, undefined_target) = match self {
            Self::Covariate => (
                "covariate",
                "covariate model has no residual degrees of freedom",
                "covariate-adjusted target statistic is undefined or non-finite",
            ),
            Self::Repeated => (
                "repeated",
                "repeated full model has no residual degrees of freedom",
                "repeated target statistic is undefined or non-finite",
            ),
        };
        let row_count = design.len();
        let column_count = design[0].len();
        if row_count <= column_count + absorbed {
            return Err(CohortInferenceError::InvalidInput(
                no_degrees_of_freedom.into(),
            ));
        }
        let mut cross = vec![vec![0.0; column_count]; column_count];
        let mut rhs = vec![0.0; column_count];
        for (row, value) in design.iter().zip(outcome) {
            for left in 0..column_count {
                rhs[left] += row[left] * value;
                for right in 0..column_count {
                    cross[left][right] += row[left] * row[right];
                }
            }
        }
        let coefficients = crate::numeric::solve_design(&cross, &rhs, model)?;
        let fitted = design
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&coefficients)
                    .map(|(x, beta)| x * beta)
                    .sum()
            })
            .collect::<Vec<f64>>();
        let residual_degrees_of_freedom = row_count - column_count - absorbed;
        let Some(target_index) = target_index else {
            return Ok(OlsFit {
                fitted,
                target_coefficient: 0.0,
                target_standard_error: 0.0,
                target_statistic: 0.0,
                residual_degrees_of_freedom,
            });
        };
        let rss = outcome
            .iter()
            .zip(&fitted)
            .map(|(actual, predicted)| (actual - predicted).powi(2))
            .sum::<f64>();
        let variance = rss / residual_degrees_of_freedom as f64;
        let mut unit = vec![0.0; column_count];
        unit[target_index] = 1.0;
        let inverse_column = crate::numeric::solve_design(&cross, &unit, model)?;
        let standard_error = (variance * inverse_column[target_index]).sqrt();
        let coefficient = coefficients[target_index];
        let statistic = coefficient / standard_error;
        if !coefficient.is_finite()
            || !standard_error.is_finite()
            || standard_error <= 0.0
            || !statistic.is_finite()
        {
            return Err(CohortInferenceError::NumericalFailure(
                undefined_target.into(),
            ));
        }
        Ok(OlsFit {
            fitted,
            target_coefficient: coefficient,
            target_standard_error: standard_error,
            target_statistic: statistic,
            residual_degrees_of_freedom,
        })
    }
}
