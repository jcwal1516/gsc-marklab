//! Training-only regularized logistic likelihood on a contiguous standardized matrix.
use super::super::{GroupedConformalModel, GroupedConformalSpec};
use marklab_numerics::NumericsError;
use std::time::Instant;

pub(super) fn fit(
    spec: &GroupedConformalSpec,
    deadline: Instant,
) -> Result<GroupedConformalModel, NumericsError> {
    let d = spec.feature_names.len();
    let training = spec
        .patients
        .iter()
        .filter(|p| p.split == "train")
        .collect::<Vec<_>>();
    let mut mean = vec![0.0; d];
    for p in &training {
        for (m, x) in mean.iter_mut().zip(&p.features) {
            *m += x;
        }
    }
    for m in &mut mean {
        *m /= training.len() as f64;
    }
    let mut sd = vec![0.0; d];
    for p in &training {
        for ((sum, x), m) in sd.iter_mut().zip(&p.features).zip(&mean) {
            *sum += (x - m) * (x - m);
        }
    }
    for s in &mut sd {
        *s = (*s / training.len() as f64).sqrt();
    }
    if mean.iter().any(|x| !x.is_finite()) || sd.iter().any(|x| !x.is_finite() || *x <= 1e-14) {
        return Err(NumericsError::Invalid(
            "every training feature must have finite population SD above 1e-14".into(),
        ));
    }
    let mut matrix = Vec::with_capacity(training.len() * d);
    let mut labels = Vec::with_capacity(training.len());
    for p in training {
        matrix.extend(
            p.features
                .iter()
                .zip(&mean)
                .zip(&sd)
                .map(|((x, m), s)| (x - m) / s),
        );
        labels.push(f64::from(p.label));
    }
    let parameters = crate::logistic_fit::fit(&matrix, &labels, d, spec.l2_penalty, deadline)?;
    Ok(GroupedConformalModel {
        training_mean: mean,
        training_population_sd: sd,
        intercept: parameters[0],
        coefficients: parameters[1..].to_vec(),
        l2_penalty: spec.l2_penalty,
    })
}
