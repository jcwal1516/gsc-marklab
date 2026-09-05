//! Native paired-Gaussian pCCA EM; no process, file, or ambient configuration access.

use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{matrix, PccaEmDesign, PccaEmSpec};
use crate::BayesError;

static IMPLEMENTATION_SHA256: LazyLock<String> = LazyLock::new(|| {
    let mut hash = Sha256::new();
    hash.update(
        concat!(
            include_str!("native.rs"),
            include_str!("matrix.rs"),
            include_str!("../pcca_em.rs"),
            include_str!("../linalg.rs")
        )
        .as_bytes(),
    );
    format!("{:x}", hash.finalize())
});

/// Truthful native implementation identity.
#[derive(Debug, Serialize)]
pub struct NativePccaBackend {
    pub name: &'static str,
    pub version: &'static str,
    pub implementation_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PccaValidatedDesign {
    #[serde(flatten)]
    pub design: PccaEmDesign,
    pub validation_status: &'static str,
    pub paired_row_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PccaStandardization {
    pub fit_split: &'static str,
    pub fit_row_count: usize,
    pub mean_x: Vec<f64>,
    pub scale_x: Vec<f64>,
    pub mean_y: Vec<f64>,
    pub scale_y: Vec<f64>,
}

#[derive(Debug, Serialize)]
pub struct PccaParameters {
    pub loadings_x: Vec<Vec<f64>>,
    pub loadings_y: Vec<Vec<f64>>,
    pub noise_diagonal_x: Vec<f64>,
    pub noise_diagonal_y: Vec<f64>,
}

#[derive(Debug, Serialize)]
pub struct PccaPosteriorScore {
    pub entity_id: String,
    pub split: String,
    pub mean: Vec<f64>,
}

#[derive(Debug, Serialize)]
pub struct PccaDiagnostics {
    pub converged: bool,
    pub iterations: usize,
    pub log_likelihood_trace: Vec<f64>,
    pub monotone_violations: u32,
    pub noise_floor: f64,
    pub regularization: f64,
}

/// Complete version-2 native pCCA result with the version-1 scientific fields retained.
#[derive(Debug, Serialize)]
pub struct PccaEmFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: NativePccaBackend,
    pub request_sha256: String,
    pub design: PccaValidatedDesign,
    pub standardization: PccaStandardization,
    pub parameters: PccaParameters,
    pub posterior_scores: Vec<PccaPosteriorScore>,
    pub canonical_correlations: Vec<f64>,
    pub heldout_row_count: usize,
    pub heldout_cross_view_rmse_y_from_x: f64,
    pub diagnostics: PccaDiagnostics,
    pub claim_status: &'static str,
}

/// Fit paired patient-level Gaussian views with training-only standardization and diagonal-noise EM.
///
/// The implementation preserves the legacy SVD initialization, exact EM objective and updates,
/// convergence rule, sign convention, complete per-row joint posterior scores, training canonical
/// correlations, and held-out Y-from-X RMSE. A training Gram matrix is prepared once; EM iteration
/// work is O((dx+dy)^2 * latent), while the final reporting pass remains linear in patient count.
/// Memory is O(rows * (dx+dy+latent) + (dx+dy)^2).
pub fn fit_pcca_em(spec: PccaEmSpec) -> Result<PccaEmFit, BayesError> {
    let started = Instant::now();
    let spec = spec.validated()?;
    let deadline = started + Duration::from_secs(spec.timeout_seconds);
    let train_indices = spec
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.split == "train").then_some(index))
        .collect::<Vec<_>>();
    let test_indices = spec
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.split == "test").then_some(index))
        .collect::<Vec<_>>();
    let dx = spec.design.modality_x.feature_names.len();
    let dy = spec.design.modality_y.feature_names.len();
    let latent = spec.latent_dimensions;
    let mean_x = column_means(&spec.rows, &train_indices, dx, |row| &row.x);
    let mean_y = column_means(&spec.rows, &train_indices, dy, |row| &row.y);
    let scale_x = column_scales(&spec.rows, &train_indices, &mean_x, dx, |row| &row.x)?;
    let scale_y = column_scales(&spec.rows, &train_indices, &mean_y, dy, |row| &row.y)?;
    let standardized_x = standardize(&spec, &mean_x, &scale_x, true);
    let standardized_y = standardize(&spec, &mean_y, &scale_y, false);
    let train_count = train_indices.len();
    let dimension = dx + dy;
    let mut observations = vec![0.0; train_count * dimension];
    for (train_row, &source_row) in train_indices.iter().enumerate() {
        observations[train_row * dimension..train_row * dimension + dx]
            .copy_from_slice(&standardized_x[source_row * dx..(source_row + 1) * dx]);
        observations[train_row * dimension + dx..(train_row + 1) * dimension]
            .copy_from_slice(&standardized_y[source_row * dy..(source_row + 1) * dy]);
    }
    check_deadline(deadline)?;
    let gram = training_gram(&observations, train_count, dimension, deadline)?;
    let mut cross = vec![0.0; dx * dy];
    for x_feature in 0..dx {
        for y_feature in 0..dy {
            cross[x_feature * dy + y_feature] =
                gram[x_feature * dimension + dx + y_feature] / train_count as f64;
        }
    }
    drop(observations);
    let (left, right) = matrix::leading_singular_vectors(&cross, dx, dy, latent)?;
    let mut loadings = vec![0.0; dimension * latent];
    for row in 0..dx {
        for component in 0..latent {
            loadings[row * latent + component] = 0.5 * left[row * latent + component];
        }
    }
    for row in 0..dy {
        for component in 0..latent {
            loadings[(dx + row) * latent + component] = 0.5 * right[row * latent + component];
        }
    }
    let mut noise = vec![0.75; dimension];
    let mut trace = Vec::with_capacity(spec.maximum_iterations.min(4096));
    let mut converged = false;
    let mut monotone_violations = 0u32;
    for _ in 0..spec.maximum_iterations {
        check_deadline(deadline)?;
        let (sum_cross, sum_second) =
            em_sufficient_moments(&gram, train_count, dimension, &loadings, latent, &noise)?;
        let mut regularized_second = sum_second.clone();
        for component in 0..latent {
            regularized_second[component * latent + component] += spec.regularization;
        }
        let inverse_second = matrix::inverse_spd(&regularized_second, latent)?;
        let mut updated_loadings = vec![0.0; dimension * latent];
        for feature in 0..dimension {
            for component in 0..latent {
                updated_loadings[feature * latent + component] = (0..latent)
                    .map(|inner| {
                        sum_cross[feature * latent + inner]
                            * inverse_second[inner * latent + component]
                    })
                    .sum();
            }
        }
        let mut updated_noise = vec![0.0; dimension];
        for feature in 0..dimension {
            let observed_square = gram[feature * dimension + feature];
            let cross_term = (0..latent)
                .map(|component| {
                    updated_loadings[feature * latent + component]
                        * sum_cross[feature * latent + component]
                })
                .sum::<f64>();
            let mut loading_second = 0.0;
            for left_component in 0..latent {
                for right_component in 0..latent {
                    loading_second += updated_loadings[feature * latent + left_component]
                        * sum_second[left_component * latent + right_component]
                        * updated_loadings[feature * latent + right_component];
                }
            }
            let residual =
                (observed_square - 2.0 * cross_term + loading_second) / train_count as f64;
            updated_noise[feature] = residual.max(spec.noise_floor);
        }
        let value = log_likelihood(
            &gram,
            train_count,
            dimension,
            &updated_loadings,
            latent,
            &updated_noise,
        )?;
        if trace
            .last()
            .is_some_and(|previous| value < *previous - 1e-7)
        {
            monotone_violations += 1;
        }
        trace.push(value);
        let loading_change = updated_loadings
            .iter()
            .zip(&loadings)
            .map(|(new, old)| (new - old).abs())
            .fold(0.0, f64::max);
        let noise_change = updated_noise
            .iter()
            .zip(&noise)
            .map(|(new, old)| (new - old).abs())
            .fold(0.0, f64::max);
        loadings = updated_loadings;
        noise = updated_noise;
        if loading_change.max(noise_change) <= spec.convergence_tolerance {
            converged = true;
            break;
        }
    }
    if trace.is_empty() || trace.iter().any(|value| !value.is_finite()) {
        return Err(BayesError::InvalidSpec(
            "pCCA EM produced no finite likelihood evidence".into(),
        ));
    }
    for component in 0..latent {
        let pivot = (0..dx)
            .max_by(|left, right| {
                loadings[*left * latent + component]
                    .abs()
                    .total_cmp(&loadings[*right * latent + component].abs())
            })
            .expect("positive x dimension");
        if loadings[pivot * latent + component] < 0.0 {
            for row in 0..dimension {
                loadings[row * latent + component] *= -1.0;
            }
        }
    }
    let loadings_x = loadings[..dx * latent].to_vec();
    let loadings_y = loadings[dx * latent..].to_vec();
    let noise_x = noise[..dx].to_vec();
    let noise_y = noise[dx..].to_vec();
    let (scores_x, _) = posterior_scores(
        &standardized_x,
        spec.rows.len(),
        dx,
        &loadings_x,
        latent,
        &noise_x,
    )?;
    let (scores_y, _) = posterior_scores(
        &standardized_y,
        spec.rows.len(),
        dy,
        &loadings_y,
        latent,
        &noise_y,
    )?;
    let mut all_observations = vec![0.0; spec.rows.len() * dimension];
    for row in 0..spec.rows.len() {
        all_observations[row * dimension..row * dimension + dx]
            .copy_from_slice(&standardized_x[row * dx..(row + 1) * dx]);
        all_observations[row * dimension + dx..(row + 1) * dimension]
            .copy_from_slice(&standardized_y[row * dy..(row + 1) * dy]);
    }
    let (joint_scores, _) = posterior_scores(
        &all_observations,
        spec.rows.len(),
        dimension,
        &loadings,
        latent,
        &noise,
    )?;
    let canonical_correlations = (0..latent)
        .map(|component| {
            correlation(
                &train_indices
                    .iter()
                    .map(|row| scores_x[row * latent + component])
                    .collect::<Vec<_>>(),
                &train_indices
                    .iter()
                    .map(|row| scores_y[row * latent + component])
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut squared_error = 0.0;
    for &row in &test_indices {
        for y_feature in 0..dy {
            let standardized_prediction = (0..latent)
                .map(|component| {
                    scores_x[row * latent + component] * loadings_y[y_feature * latent + component]
                })
                .sum::<f64>();
            let prediction = standardized_prediction * scale_y[y_feature] + mean_y[y_feature];
            squared_error += (prediction - spec.rows[row].y[y_feature]).powi(2);
        }
    }
    let heldout_rmse = (squared_error / (test_indices.len() * dy) as f64).sqrt();
    let implementation_sha256 = LazyLock::force(&IMPLEMENTATION_SHA256).clone();
    let request_sha256 = input_identity(&implementation_sha256, &spec);
    let fit = PccaEmFit {
        format: "marklab.probabilistic_cca",
        version: 2,
        backend: NativePccaBackend {
            name: "marklab-rust",
            version: env!("CARGO_PKG_VERSION"),
            implementation_sha256,
        },
        request_sha256,
        design: PccaValidatedDesign {
            design: spec.design,
            validation_status: "passed",
            paired_row_count: spec.rows.len(),
        },
        standardization: PccaStandardization {
            fit_split: "train_only",
            fit_row_count: train_count,
            mean_x,
            scale_x,
            mean_y,
            scale_y,
        },
        parameters: PccaParameters {
            loadings_x: rows(&loadings_x, dx, latent),
            loadings_y: rows(&loadings_y, dy, latent),
            noise_diagonal_x: noise_x,
            noise_diagonal_y: noise_y,
        },
        posterior_scores: spec
            .rows
            .into_iter()
            .enumerate()
            .map(|(row, input)| PccaPosteriorScore {
                entity_id: input.entity_id,
                split: input.split,
                mean: joint_scores[row * latent..(row + 1) * latent].to_vec(),
            })
            .collect(),
        canonical_correlations,
        heldout_row_count: test_indices.len(),
        heldout_cross_view_rmse_y_from_x: heldout_rmse,
        diagnostics: PccaDiagnostics {
            converged,
            iterations: trace.len(),
            log_likelihood_trace: trace,
            monotone_violations,
            noise_floor: spec.noise_floor,
            regularization: spec.regularization,
        },
        claim_status: "experimental_synthetic_paired_gaussian_pcca",
    };
    if serde_json::to_vec(&fit)?.len() > 16 * 1024 * 1024 {
        return Err(BayesError::InvalidSpec("pCCA result exceeds 16 MiB".into()));
    }
    check_deadline(deadline)?;
    Ok(fit)
}

fn column_means(
    rows: &[super::PccaEmRow],
    train: &[usize],
    dimension: usize,
    values: impl Fn(&super::PccaEmRow) -> &[f64],
) -> Vec<f64> {
    (0..dimension)
        .map(|column| {
            train
                .iter()
                .map(|&row| values(&rows[row])[column])
                .sum::<f64>()
                / train.len() as f64
        })
        .collect()
}

fn column_scales(
    rows: &[super::PccaEmRow],
    train: &[usize],
    means: &[f64],
    dimension: usize,
    values: impl Fn(&super::PccaEmRow) -> &[f64],
) -> Result<Vec<f64>, BayesError> {
    let scales = (0..dimension)
        .map(|column| {
            (train
                .iter()
                .map(|&row| (values(&rows[row])[column] - means[column]).powi(2))
                .sum::<f64>()
                / train.len() as f64)
                .sqrt()
        })
        .collect::<Vec<_>>();
    if scales
        .iter()
        .any(|scale| !scale.is_finite() || *scale <= 0.0)
    {
        return Err(BayesError::InvalidSpec(
            "pCCA training features must vary".into(),
        ));
    }
    Ok(scales)
}

fn standardize(spec: &PccaEmSpec, means: &[f64], scales: &[f64], x: bool) -> Vec<f64> {
    let dimension = means.len();
    let mut output = Vec::with_capacity(spec.rows.len() * dimension);
    for row in &spec.rows {
        let values = if x { &row.x } else { &row.y };
        output.extend(
            values
                .iter()
                .zip(means.iter().zip(scales))
                .map(|(value, (mean, scale))| (value - mean) / scale),
        );
    }
    output
}

fn training_gram(
    observations: &[f64],
    count: usize,
    dimension: usize,
    deadline: Instant,
) -> Result<Vec<f64>, BayesError> {
    let mut gram = vec![0.0; dimension * dimension];
    for left in 0..dimension {
        check_deadline(deadline)?;
        for right in 0..=left {
            let value = (0..count)
                .map(|row| {
                    observations[row * dimension + left] * observations[row * dimension + right]
                })
                .sum::<f64>();
            gram[left * dimension + right] = value;
            gram[right * dimension + left] = value;
        }
    }
    if gram.iter().any(|value| !value.is_finite()) {
        return Err(BayesError::InvalidSpec(
            "pCCA training sufficient statistic is nonfinite".into(),
        ));
    }
    Ok(gram)
}

struct PosteriorSystem {
    precision_loadings: Vec<f64>,
    middle: Vec<f64>,
    covariance: Vec<f64>,
    linear_map: Vec<f64>,
}

fn posterior_system(
    dimension: usize,
    loadings: &[f64],
    latent: usize,
    noise: &[f64],
) -> Result<PosteriorSystem, BayesError> {
    let mut precision_loadings = vec![0.0; dimension * latent];
    for feature in 0..dimension {
        if !noise[feature].is_finite() || noise[feature] <= 0.0 {
            return Err(BayesError::InvalidSpec("pCCA noise is invalid".into()));
        }
        for component in 0..latent {
            precision_loadings[feature * latent + component] =
                loadings[feature * latent + component] / noise[feature];
        }
    }
    let mut middle = vec![0.0; latent * latent];
    for left in 0..latent {
        for right in 0..latent {
            middle[left * latent + right] = usize::from(left == right) as f64
                + (0..dimension)
                    .map(|feature| {
                        loadings[feature * latent + left]
                            * precision_loadings[feature * latent + right]
                    })
                    .sum::<f64>();
        }
    }
    let covariance = matrix::inverse_spd(&middle, latent)?;
    let mut linear_map = vec![0.0; dimension * latent];
    for feature in 0..dimension {
        for component in 0..latent {
            linear_map[feature * latent + component] = (0..latent)
                .map(|inner| {
                    precision_loadings[feature * latent + inner]
                        * covariance[inner * latent + component]
                })
                .sum();
        }
    }
    if precision_loadings
        .iter()
        .chain(&middle)
        .chain(&covariance)
        .chain(&linear_map)
        .any(|value| !value.is_finite())
    {
        return Err(BayesError::InvalidSpec(
            "pCCA posterior system is nonfinite".into(),
        ));
    }
    Ok(PosteriorSystem {
        precision_loadings,
        middle,
        covariance,
        linear_map,
    })
}

fn em_sufficient_moments(
    gram: &[f64],
    count: usize,
    dimension: usize,
    loadings: &[f64],
    latent: usize,
    noise: &[f64],
) -> Result<(Vec<f64>, Vec<f64>), BayesError> {
    let system = posterior_system(dimension, loadings, latent, noise)?;
    let mut sum_cross = vec![0.0; dimension * latent];
    for feature in 0..dimension {
        for component in 0..latent {
            sum_cross[feature * latent + component] = (0..dimension)
                .map(|inner| {
                    gram[feature * dimension + inner]
                        * system.linear_map[inner * latent + component]
                })
                .sum();
        }
    }
    let mut sum_second = system
        .covariance
        .iter()
        .map(|value| count as f64 * value)
        .collect::<Vec<_>>();
    for left in 0..latent {
        for right in 0..latent {
            sum_second[left * latent + right] += (0..dimension)
                .map(|feature| {
                    system.linear_map[feature * latent + left] * sum_cross[feature * latent + right]
                })
                .sum::<f64>();
        }
    }
    if sum_cross
        .iter()
        .chain(&sum_second)
        .any(|value| !value.is_finite())
    {
        return Err(BayesError::InvalidSpec(
            "pCCA EM sufficient moments are nonfinite".into(),
        ));
    }
    Ok((sum_cross, sum_second))
}

fn posterior_scores(
    observations: &[f64],
    count: usize,
    dimension: usize,
    loadings: &[f64],
    latent: usize,
    noise: &[f64],
) -> Result<(Vec<f64>, Vec<f64>), BayesError> {
    let system = posterior_system(dimension, loadings, latent, noise)?;
    let mut scores = vec![0.0; count * latent];
    for row in 0..count {
        for component in 0..latent {
            scores[row * latent + component] = (0..dimension)
                .map(|feature| {
                    observations[row * dimension + feature]
                        * system.linear_map[feature * latent + component]
                })
                .sum();
        }
    }
    if scores
        .iter()
        .chain(&system.covariance)
        .any(|value| !value.is_finite())
    {
        return Err(BayesError::InvalidSpec(
            "pCCA posterior scores are nonfinite".into(),
        ));
    }
    Ok((scores, system.covariance))
}

fn log_likelihood(
    gram: &[f64],
    count: usize,
    dimension: usize,
    loadings: &[f64],
    latent: usize,
    noise: &[f64],
) -> Result<f64, BayesError> {
    // Matrix determinant lemma and Woodbury evaluate the same Gaussian objective from X'X.
    let system = posterior_system(dimension, loadings, latent, noise)?;
    let lower = matrix::cholesky(&system.middle, latent)?;
    let log_determinant = noise.iter().map(|value| value.ln()).sum::<f64>()
        + 2.0
            * (0..latent)
                .map(|index| lower[index * latent + index].ln())
                .sum::<f64>();
    let diagonal_quadratic = (0..dimension)
        .map(|feature| gram[feature * dimension + feature] / noise[feature])
        .sum::<f64>();
    let mut gram_precision_loadings = vec![0.0; dimension * latent];
    for feature in 0..dimension {
        for component in 0..latent {
            gram_precision_loadings[feature * latent + component] = (0..dimension)
                .map(|inner| {
                    gram[feature * dimension + inner]
                        * system.precision_loadings[inner * latent + component]
                })
                .sum();
        }
    }
    let mut correction = 0.0;
    for left in 0..latent {
        for right in 0..latent {
            let projected_gram = (0..dimension)
                .map(|feature| {
                    system.precision_loadings[feature * latent + left]
                        * gram_precision_loadings[feature * latent + right]
                })
                .sum::<f64>();
            correction += projected_gram * system.covariance[right * latent + left];
        }
    }
    let quadratic = diagonal_quadratic - correction;
    let value = -0.5
        * (count as f64 * (dimension as f64 * (2.0 * std::f64::consts::PI).ln() + log_determinant)
            + quadratic);
    if !value.is_finite() {
        return Err(BayesError::InvalidSpec(
            "pCCA log likelihood is nonfinite".into(),
        ));
    }
    Ok(value)
}

fn correlation(left: &[f64], right: &[f64]) -> Result<f64, BayesError> {
    let mean_left = left.iter().sum::<f64>() / left.len() as f64;
    let mean_right = right.iter().sum::<f64>() / right.len() as f64;
    let covariance = left
        .iter()
        .zip(right)
        .map(|(a, b)| (a - mean_left) * (b - mean_right))
        .sum::<f64>();
    let left_square = left
        .iter()
        .map(|value| (value - mean_left).powi(2))
        .sum::<f64>();
    let right_square = right
        .iter()
        .map(|value| (value - mean_right).powi(2))
        .sum::<f64>();
    let value = covariance / (left_square * right_square).sqrt();
    if !value.is_finite() {
        return Err(BayesError::InvalidSpec(
            "pCCA canonical correlation is undefined".into(),
        ));
    }
    Ok(value)
}

fn rows(matrix: &[f64], count: usize, columns: usize) -> Vec<Vec<f64>> {
    (0..count)
        .map(|row| matrix[row * columns..(row + 1) * columns].to_vec())
        .collect()
}

fn check_deadline(deadline: Instant) -> Result<(), BayesError> {
    if Instant::now() >= deadline {
        Err(BayesError::InvalidSpec("pCCA deadline exceeded".into()))
    } else {
        Ok(())
    }
}

fn input_identity(implementation: &str, spec: &PccaEmSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(b"marklab.pcca-em.spec.v1\0");
    hash_text(&mut hash, implementation);
    hash_text(&mut hash, &spec.design.entity_level);
    for modality in [&spec.design.modality_x, &spec.design.modality_y] {
        hash_text(&mut hash, &modality.id);
        hash_text(&mut hash, &modality.measurement_status);
        hash_text(&mut hash, &modality.likelihood);
        hash.update((modality.feature_names.len() as u64).to_le_bytes());
        for feature in &modality.feature_names {
            hash_text(&mut hash, feature);
        }
    }
    hash_text(&mut hash, &spec.design.missingness_assumption);
    match &spec.design.coordinate_frame {
        Some(frame) => {
            hash.update([1]);
            hash_text(&mut hash, frame);
        }
        None => hash.update([0]),
    }
    hash.update((spec.rows.len() as u64).to_le_bytes());
    for row in &spec.rows {
        hash_text(&mut hash, &row.entity_id);
        hash_text(&mut hash, &row.split);
        for value in row.x.iter().chain(&row.y) {
            hash.update(value.to_bits().to_le_bytes());
        }
    }
    hash.update((spec.latent_dimensions as u64).to_le_bytes());
    hash.update(spec.regularization.to_bits().to_le_bytes());
    hash.update(spec.noise_floor.to_bits().to_le_bytes());
    hash.update((spec.maximum_iterations as u64).to_le_bytes());
    hash.update(spec.convergence_tolerance.to_bits().to_le_bytes());
    hash.update(spec.timeout_seconds.to_le_bytes());
    format!("{:x}", hash.finalize())
}

fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}
