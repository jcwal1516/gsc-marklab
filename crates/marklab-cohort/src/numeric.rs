use crate::{compensated_sum, CohortInferenceError};

pub(crate) struct MeanStandardError {
    pub(crate) count: usize,
    pub(crate) mean: f64,
    pub(crate) standard_error: f64,
    pub(crate) degrees_of_freedom: f64,
}

pub(crate) fn mean_standard_error(
    values: &[f64],
) -> Result<MeanStandardError, CohortInferenceError> {
    if values.len() < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "at least two patient effects are required".into(),
        ));
    }
    let mean = stable_mean(values)?;
    let variance = compensated_sum(values.iter().map(|value| {
        let centered = value - mean;
        centered * centered
    })) / (values.len() - 1) as f64;
    let standard_error = (variance / values.len() as f64).sqrt();
    if !mean.is_finite() || !standard_error.is_finite() || standard_error == 0.0 {
        return Err(CohortInferenceError::NumericalFailure(
            "patient effects have zero or non-finite standard error".into(),
        ));
    }
    Ok(MeanStandardError {
        count: values.len(),
        mean: if mean == 0.0 { 0.0 } else { mean },
        standard_error,
        degrees_of_freedom: (values.len() - 1) as f64,
    })
}

pub(crate) fn stable_mean(values: &[f64]) -> Result<f64, CohortInferenceError> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return Err(CohortInferenceError::InvalidInput(
            "stable mean requires at least one finite value".into(),
        ));
    }
    let scale = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let mean = if scale == 0.0 {
        0.0
    } else {
        (compensated_sum(values.iter().map(|value| value / scale)) / values.len() as f64) * scale
    };
    if !mean.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "stable mean produced a non-finite result".into(),
        ));
    }
    Ok(if mean == 0.0 { 0.0 } else { mean })
}

pub(crate) struct WelchContrast {
    pub(crate) group_a_count: usize,
    pub(crate) group_b_count: usize,
    pub(crate) group_a_mean: f64,
    pub(crate) group_b_mean: f64,
    pub(crate) effect: f64,
    pub(crate) studentized: f64,
}

pub(crate) fn welch_contrast(
    values: &[f64],
    labels: &[bool],
) -> Result<WelchContrast, CohortInferenceError> {
    let (group_a_count, group_a_mean) = stable_group_mean(values, labels, true);
    let (group_b_count, group_b_mean) = stable_group_mean(values, labels, false);
    if group_a_count < 2 || group_b_count < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "each group must contain at least two patients".into(),
        ));
    }
    let group_a_variance = sample_group_variance(values, labels, true, group_a_mean, group_a_count);
    let group_b_variance =
        sample_group_variance(values, labels, false, group_b_mean, group_b_count);
    let standard_error =
        (group_a_variance / group_a_count as f64 + group_b_variance / group_b_count as f64).sqrt();
    if !standard_error.is_finite() || standard_error == 0.0 {
        return Err(CohortInferenceError::NumericalFailure(
            "group contrast has zero or non-finite standard error".into(),
        ));
    }
    let effect = group_a_mean - group_b_mean;
    let studentized = effect / standard_error;
    if !effect.is_finite() || !studentized.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "group contrast produced a non-finite result".into(),
        ));
    }
    Ok(WelchContrast {
        group_a_count,
        group_b_count,
        group_a_mean,
        group_b_mean,
        effect,
        studentized,
    })
}

fn stable_group_mean(values: &[f64], labels: &[bool], target_group_a: bool) -> (usize, f64) {
    let selected = values
        .iter()
        .zip(labels)
        .filter(|(_, is_group_a)| **is_group_a == target_group_a);
    let count = selected.clone().count();
    let scale = selected
        .clone()
        .map(|(value, _)| value.abs())
        .fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return (count, 0.0);
    }
    let normalized_sum = compensated_sum(selected.map(|(value, _)| value / scale));
    (count, (normalized_sum / count as f64) * scale)
}

fn sample_group_variance(
    values: &[f64],
    labels: &[bool],
    target_group_a: bool,
    mean: f64,
    count: usize,
) -> f64 {
    compensated_sum(
        values
            .iter()
            .zip(labels)
            .filter(|(_, is_group_a)| **is_group_a == target_group_a)
            .map(|(value, _)| {
                let centered = value - mean;
                centered * centered
            }),
    ) / (count - 1) as f64
}
