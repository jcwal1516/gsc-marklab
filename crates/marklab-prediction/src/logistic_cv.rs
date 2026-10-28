//! Nested, grouped cross-validation of L2-penalised logistic regression.
//!
//! Groups (such as patients) are dealt to folds whole, stratified by whether a group has any
//! positive row, after a seeded shuffle. Within each outer training set an inner
//! cross-validation chooses `C` by mean inner-fold AUROC, the model is refitted on the whole
//! outer training set with that `C`, and the outer test fold is predicted.

use std::collections::BTreeMap;

use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

use crate::logistic::{ClassWeight, LogisticError, LogisticProblem};

/// Largest number of outer or inner folds.
pub const MAXIMUM_LOGISTIC_FOLDS: usize = 20;
/// Largest number of candidate `C` values.
pub const MAXIMUM_LOGISTIC_C_VALUES: usize = 32;

/// Cross-validation settings.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LogisticCvSpec {
    /// Candidate inverse penalty strengths, strictly increasing.
    pub c_values: Vec<f64>,
    pub class_weight: ClassWeight,
    /// Outer folds, each predicted by a model that never saw it.
    pub folds: usize,
    /// Inner folds that choose `C` within each outer training set.
    pub inner_folds: usize,
    /// Seed of the fold shuffles.
    pub seed: u64,
}

/// One outer fold.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LogisticCvFold {
    pub fold: usize,
    pub training_rows: usize,
    pub test_rows: usize,
    pub test_positives: usize,
    /// Mean inner-fold AUROC of each candidate `C`, in the order of `c_values`.
    pub inner_mean_auroc: Vec<f64>,
    /// The first `C` with the highest mean inner AUROC.
    pub selected_c: f64,
    pub auroc: f64,
    pub average_precision: f64,
}

/// Out-of-fold prediction of one row.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LogisticCvPrediction {
    pub row: usize,
    pub fold: usize,
    pub probability: f64,
}

/// Result of [`cross_validate_logistic`].
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LogisticCvResult {
    pub folds: Vec<LogisticCvFold>,
    pub mean_fold_auroc: f64,
    pub mean_fold_average_precision: f64,
    /// AUROC of all out-of-fold predictions together.
    pub pooled_auroc: f64,
    pub pooled_average_precision: f64,
    /// One prediction per row, in row order.
    pub predictions: Vec<LogisticCvPrediction>,
}

/// Cross-validate on row-major `features` (`labels.len()` rows of `columns` values), keeping the
/// rows of each `groups` value in one fold.
pub fn cross_validate_logistic(
    features: &[f64],
    columns: usize,
    labels: &[bool],
    groups: &[usize],
    spec: &LogisticCvSpec,
) -> Result<LogisticCvResult, LogisticError> {
    validate_spec(spec)?;
    let rows = labels.len();
    if groups.len() != rows || columns == 0 || rows.checked_mul(columns) != Some(features.len()) {
        return Err(LogisticError::Invalid(
            "features, labels and groups must describe the same rows".to_owned(),
        ));
    }
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let all: Vec<usize> = (0..rows).collect();
    let outer = assign_folds(&all, labels, groups, spec.folds, &mut rng)
        .map_err(|message| LogisticError::Invalid(format!("outer folds: {message}")))?;
    let mut folds = Vec::with_capacity(spec.folds);
    let mut predictions = vec![None; rows];
    for (fold, test) in outer.iter().enumerate() {
        let training = complement(rows, test);
        let inner = assign_folds(&training, labels, groups, spec.inner_folds, &mut rng).map_err(
            |message| {
                LogisticError::Invalid(format!("inner folds of outer fold {fold}: {message}"))
            },
        )?;
        let mut inner_auroc = vec![0.0; spec.c_values.len()];
        for inner_test in &inner {
            let inner_training: Vec<usize> = training
                .iter()
                .copied()
                .filter(|row| inner_test.binary_search(row).is_err())
                .collect();
            let scores = fit_and_score(
                features,
                columns,
                labels,
                &inner_training,
                inner_test,
                spec,
                &spec.c_values,
            )?;
            let truth = select(labels, inner_test);
            for (total, scores) in inner_auroc.iter_mut().zip(scores) {
                *total += area_under_roc(&truth, &scores).ok_or_else(|| {
                    LogisticError::Invalid("an inner test fold lacks a class".to_owned())
                })?;
            }
        }
        for total in &mut inner_auroc {
            *total /= inner.len() as f64;
        }
        let best = inner_auroc
            .iter()
            .enumerate()
            .fold(0, |best, (index, &value)| {
                if value > inner_auroc[best] {
                    index
                } else {
                    best
                }
            });
        let selected_c = spec.c_values[best];
        let scores = fit_and_score(
            features,
            columns,
            labels,
            &training,
            test,
            spec,
            &[selected_c],
        )?
        .remove(0);
        let truth = select(labels, test);
        let auroc = area_under_roc(&truth, &scores);
        let average_precision = average_precision(&truth, &scores);
        let (Some(auroc), Some(average_precision)) = (auroc, average_precision) else {
            return Err(LogisticError::Invalid(format!(
                "outer test fold {fold} lacks a class"
            )));
        };
        for (&row, &probability) in test.iter().zip(&scores) {
            predictions[row] = Some(LogisticCvPrediction {
                row,
                fold,
                probability,
            });
        }
        folds.push(LogisticCvFold {
            fold,
            training_rows: training.len(),
            test_rows: test.len(),
            test_positives: truth.iter().filter(|&&label| label).count(),
            inner_mean_auroc: inner_auroc,
            selected_c,
            auroc,
            average_precision,
        });
    }
    let predictions: Vec<LogisticCvPrediction> = predictions
        .into_iter()
        .map(|prediction| prediction.expect("outer folds partition the rows"))
        .collect();
    let probabilities: Vec<f64> = predictions.iter().map(|p| p.probability).collect();
    let mean = |values: Vec<f64>| values.iter().sum::<f64>() / values.len() as f64;
    Ok(LogisticCvResult {
        mean_fold_auroc: mean(folds.iter().map(|f| f.auroc).collect()),
        mean_fold_average_precision: mean(folds.iter().map(|f| f.average_precision).collect()),
        pooled_auroc: area_under_roc(labels, &probabilities).expect("both classes are present"),
        pooled_average_precision: average_precision(labels, &probabilities)
            .expect("both classes are present"),
        folds,
        predictions,
    })
}

/// Area under the ROC curve for positive `labels`, counting tied scores as one half (the
/// Mann–Whitney statistic). `None` unless both classes are present.
pub fn area_under_roc(labels: &[bool], scores: &[f64]) -> Option<f64> {
    let positives = labels.iter().filter(|&&label| label).count();
    let negatives = labels.len() - positives;
    if positives == 0 || negatives == 0 || scores.len() != labels.len() {
        return None;
    }
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| scores[a].total_cmp(&scores[b]));
    let mut positive_rank_sum = 0.0;
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && scores[order[end]] == scores[order[start]] {
            end += 1;
        }
        let midrank = (start + end + 1) as f64 / 2.0;
        let tied_positives = order[start..end].iter().filter(|&&i| labels[i]).count();
        positive_rank_sum += midrank * tied_positives as f64;
        start = end;
    }
    let positives = positives as f64;
    Some((positive_rank_sum - positives * (positives + 1.0) / 2.0) / (positives * negatives as f64))
}

/// Average precision, `Σ (Rₖ − Rₖ₋₁) Pₖ` over decreasing score thresholds, as in
/// scikit-learn's `average_precision_score`. `None` without a positive label.
pub fn average_precision(labels: &[bool], scores: &[f64]) -> Option<f64> {
    let positives = labels.iter().filter(|&&label| label).count();
    if positives == 0 || scores.len() != labels.len() {
        return None;
    }
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    let (mut true_positives, mut seen, mut recall, mut total) = (0usize, 0usize, 0.0, 0.0);
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && scores[order[end]] == scores[order[start]] {
            end += 1;
        }
        true_positives += order[start..end].iter().filter(|&&i| labels[i]).count();
        seen += end - start;
        let next_recall = true_positives as f64 / positives as f64;
        total += (next_recall - recall) * true_positives as f64 / seen as f64;
        recall = next_recall;
        start = end;
    }
    Some(total)
}

fn validate_spec(spec: &LogisticCvSpec) -> Result<(), LogisticError> {
    let invalid = |message: String| Err(LogisticError::Invalid(message));
    if spec.c_values.is_empty() || spec.c_values.len() > MAXIMUM_LOGISTIC_C_VALUES {
        return invalid(format!(
            "between 1 and {MAXIMUM_LOGISTIC_C_VALUES} C values are required"
        ));
    }
    if !spec.c_values.iter().all(|c| c.is_finite() && *c > 0.0)
        || !spec.c_values.windows(2).all(|pair| pair[0] < pair[1])
    {
        return invalid("C values must be finite, positive and strictly increasing".to_owned());
    }
    for (name, value) in [("folds", spec.folds), ("inner_folds", spec.inner_folds)] {
        if !(2..=MAXIMUM_LOGISTIC_FOLDS).contains(&value) {
            return invalid(format!(
                "{name} must be between 2 and {MAXIMUM_LOGISTIC_FOLDS}"
            ));
        }
    }
    Ok(())
}

/// Deal the groups of `rows` to `folds` folds: groups with a positive row first, then the rest,
/// each list shuffled and dealt round-robin. Returns each fold's rows in increasing order.
fn assign_folds(
    rows: &[usize],
    labels: &[bool],
    groups: &[usize],
    folds: usize,
    rng: &mut ChaCha20Rng,
) -> Result<Vec<Vec<usize>>, String> {
    let mut positive_group: BTreeMap<usize, bool> = BTreeMap::new();
    for &row in rows {
        *positive_group.entry(groups[row]).or_insert(false) |= labels[row];
    }
    let (mut positive, mut negative): (Vec<usize>, Vec<usize>) = positive_group
        .keys()
        .partition(|group| positive_group[group]);
    for (name, list) in [("positive", &positive), ("negative", &negative)] {
        if list.len() < folds {
            return Err(format!(
                "{} groups are {name}, fewer than the {folds} folds",
                list.len()
            ));
        }
    }
    positive.shuffle(rng);
    negative.shuffle(rng);
    let fold_of: BTreeMap<usize, usize> = positive
        .iter()
        .chain(&negative)
        .enumerate()
        .map(|(position, &group)| (group, position % folds))
        .collect();
    let mut assigned = vec![Vec::new(); folds];
    for &row in rows {
        assigned[fold_of[&groups[row]]].push(row);
    }
    Ok(assigned)
}

fn complement(rows: usize, excluded: &[usize]) -> Vec<usize> {
    (0..rows)
        .filter(|row| excluded.binary_search(row).is_err())
        .collect()
}

fn select<T: Copy>(values: &[T], rows: &[usize]) -> Vec<T> {
    rows.iter().map(|&row| values[row]).collect()
}

/// Fit on `training` for each `C` and return the predicted probabilities of `test`, one vector
/// per `C`.
fn fit_and_score(
    features: &[f64],
    columns: usize,
    labels: &[bool],
    training: &[usize],
    test: &[usize],
    spec: &LogisticCvSpec,
    c_values: &[f64],
) -> Result<Vec<Vec<f64>>, LogisticError> {
    let training_features: Vec<f64> = training
        .iter()
        .flat_map(|&row| &features[row * columns..(row + 1) * columns])
        .copied()
        .collect();
    let problem = LogisticProblem::new(
        &training_features,
        columns,
        &select(labels, training),
        spec.class_weight,
    )?;
    c_values
        .iter()
        .map(|&c| {
            let model = problem.fit(c)?;
            test.iter()
                .map(|&row| model.probability(&features[row * columns..(row + 1) * columns]))
                .collect()
        })
        .collect()
}
