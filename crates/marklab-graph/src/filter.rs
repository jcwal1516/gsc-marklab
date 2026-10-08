use crate::{GraphError, GraphSpectrum};

pub(crate) fn spectral_filter(spectrum: &GraphSpectrum, response: impl Fn(f64) -> f64) -> Vec<f64> {
    let signal = spectrum
        .eigenvectors_by_mode
        .iter()
        .zip(&spectrum.coefficients)
        .fold(
            vec![0.0; spectrum.coefficients.len()],
            |mut signal, (mode, coefficient)| {
                for (value, basis) in signal.iter_mut().zip(mode) {
                    *value += coefficient * basis;
                }
                signal
            },
        );
    spectral_filter_signal(spectrum, &signal, response)
}

pub(crate) fn spectral_filter_signal(
    spectrum: &GraphSpectrum,
    signal: &[f64],
    response: impl Fn(f64) -> f64,
) -> Vec<f64> {
    let coefficients = spectrum
        .eigenvectors_by_mode
        .iter()
        .map(|mode| {
            mode.iter()
                .zip(signal)
                .map(|(basis, value)| basis * value)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let size = coefficients.len();
    (0..size)
        .map(|node| {
            spectrum
                .eigenvalues
                .iter()
                .zip(&spectrum.eigenvectors_by_mode)
                .zip(&coefficients)
                .map(|((eigenvalue, mode), coefficient)| {
                    response(*eigenvalue) * coefficient * mode[node]
                })
                .sum()
        })
        .collect()
}

pub fn chebyshev_apply(
    scaled_matrix: &[Vec<f64>],
    input: &[f64],
    coefficients: &[f64],
) -> Result<Vec<f64>, GraphError> {
    let size = input.len();
    if size == 0
        || scaled_matrix.len() != size
        || scaled_matrix
            .iter()
            .any(|row| row.len() != size || row.iter().any(|value| !value.is_finite()))
        || input.iter().any(|value| !value.is_finite())
        || coefficients.is_empty()
        || coefficients.iter().any(|value| !value.is_finite())
    {
        return Err(GraphError::Invalid(
            "Chebyshev apply requires finite square matrix, aligned vector, and coefficients"
                .into(),
        ));
    }
    let mut previous = input.to_vec();
    let mut output = previous
        .iter()
        .map(|value| 0.5 * coefficients[0] * value)
        .collect::<Vec<_>>();
    if coefficients.len() == 1 {
        return Ok(output);
    }
    let mut current = matrix_vector(scaled_matrix, input);
    for (target, value) in output.iter_mut().zip(&current) {
        *target += coefficients[1] * value;
    }
    for &coefficient in coefficients.iter().skip(2) {
        let product = matrix_vector(scaled_matrix, &current);
        let next = product
            .iter()
            .zip(&previous)
            .map(|(product, previous)| 2.0 * product - previous)
            .collect::<Vec<_>>();
        for (target, value) in output.iter_mut().zip(&next) {
            *target += coefficient * value;
        }
        previous = current;
        current = next;
    }
    if output.iter().any(|value| !value.is_finite()) {
        return Err(GraphError::Numerical(
            "Chebyshev recurrence produced non-finite output".into(),
        ));
    }
    Ok(output)
}

fn matrix_vector(matrix: &[Vec<f64>], vector: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(vector)
                .map(|(left, right)| left * right)
                .sum()
        })
        .collect()
}

pub(crate) fn heat_chebyshev_coefficients(time: f64, lambda_max: f64, order: usize) -> Vec<f64> {
    let samples = (8 * (order + 1)).max(1_024);
    (0..=order)
        .map(|degree| {
            2.0 / samples as f64
                * (0..samples)
                    .map(|sample| {
                        let theta = std::f64::consts::PI * (sample as f64 + 0.5) / samples as f64;
                        let eigenvalue = lambda_max * (theta.cos() + 1.0) / 2.0;
                        (-time * eigenvalue).exp() * (degree as f64 * theta).cos()
                    })
                    .sum::<f64>()
        })
        .collect()
}

pub(crate) fn heat_grid_error(
    time: f64,
    lambda_max: f64,
    coefficients: &[f64],
    points: usize,
) -> f64 {
    (0..points)
        .map(|index| {
            let scaled = -1.0 + 2.0 * index as f64 / (points - 1) as f64;
            let eigenvalue = lambda_max * (scaled + 1.0) / 2.0;
            let mut previous = 1.0;
            let mut approximation = 0.5 * coefficients[0];
            if coefficients.len() > 1 {
                let mut current = scaled;
                approximation += coefficients[1] * current;
                for coefficient in coefficients.iter().skip(2) {
                    let next = 2.0 * scaled * current - previous;
                    approximation += coefficient * next;
                    previous = current;
                    current = next;
                }
            }
            (approximation - (-time * eigenvalue).exp()).abs()
        })
        .fold(0.0_f64, f64::max)
}
