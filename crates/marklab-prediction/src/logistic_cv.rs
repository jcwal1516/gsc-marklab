//! Cross-fitted ensembles of L2-penalised logistic regression, their calibration, conformal
//! p-values and evaluation.
//!
//! Fitting an ensemble deals the groups (such as patients) to `inner_folds` folds, stratified by
//! whether a group has any positive row after a seeded shuffle. Every candidate `C` is fitted on
//! each fold's complement and scored on the held-out fold; the `C` with the highest mean held-out
//! AUROC is kept, and its fold models are the members. Platt scaling is fitted to all held-out
//! margins. The held-out margins of each class, kept per member, calibrate class-conditional
//! (Mondrian) cross-conformal p-values. A new row's probability is the mean of the members'
//! calibrated probabilities.
//!
//! Evaluation repeats the whole fit within each of `folds` outer training sets and predicts the
//! outer test fold, so it measures the procedure that produces the final ensemble.

use std::collections::BTreeMap;

use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

use crate::{
    calibration::{brier_score, reliability, PlattScaling, ReliabilityBin},
    logistic::{ClassWeight, LogisticError, LogisticModel, LogisticProblem},
};

/// Largest number of outer or inner folds.
pub const MAXIMUM_LOGISTIC_FOLDS: usize = 20;
/// Largest number of candidate `C` values.
pub const MAXIMUM_LOGISTIC_C_VALUES: usize = 32;

/// Fitting and evaluation settings.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LogisticEnsembleSpec {
    /// Candidate inverse penalty strengths, strictly increasing.
    pub c_values: Vec<f64>,
    pub class_weight: ClassWeight,
    /// Outer folds that evaluate the fitting procedure.
    pub folds: usize,
    /// Folds within each fit; also the number of ensemble members.
    pub inner_folds: usize,
    /// Seed of the fold shuffles.
    pub seed: u64,
    /// Conformal significance `ε`: a class is ruled out when its p-value is at most `ε`.
    pub significance: f64,
}

/// A fitted ensemble. Deserialisation checks that its parts are consistent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "EnsembleParts")]
pub struct LogisticEnsemble {
    selected_c: f64,
    members: Vec<LogisticModel>,
    platt: PlattScaling,
    significance: f64,
    /// Held-out margins of positive rows, one increasing list per member.
    positive_margins: Vec<Vec<f64>>,
    /// Held-out margins of negative rows, one increasing list per member.
    negative_margins: Vec<Vec<f64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnsembleParts {
    selected_c: f64,
    members: Vec<LogisticModel>,
    platt: PlattScaling,
    significance: f64,
    positive_margins: Vec<Vec<f64>>,
    negative_margins: Vec<Vec<f64>>,
}

/// Which classes a conformal prediction keeps.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConformalCall {
    Positive,
    Negative,
    /// Both classes remain plausible.
    Both,
    /// Neither class is plausible: the row is unlike the calibration rows of either class.
    Neither,
}

/// Prediction for one row.
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct EnsemblePrediction {
    /// Mean of the members' Platt-calibrated probabilities of the positive class.
    pub probability: f64,
    pub p_value_positive: f64,
    pub p_value_negative: f64,
    pub call: ConformalCall,
}

/// A fitted ensemble with the held-out AUROC that chose its `C`.
#[derive(Clone, Debug, PartialEq)]
pub struct LogisticEnsembleFit {
    pub ensemble: LogisticEnsemble,
    /// Mean held-out AUROC of each candidate `C`, in the order of `c_values`.
    pub heldout_mean_auroc: Vec<f64>,
}

/// One outer fold of the evaluation.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EnsembleCvFold {
    pub fold: usize,
    pub training_rows: usize,
    pub test_rows: usize,
    pub test_positives: usize,
    pub heldout_mean_auroc: Vec<f64>,
    pub selected_c: f64,
    pub auroc: f64,
    pub average_precision: f64,
}

/// Out-of-fold prediction of one row.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EnsembleCvPrediction {
    pub row: usize,
    pub fold: usize,
    #[serde(flatten)]
    pub prediction: EnsemblePrediction,
}

/// Counts of conformal calls among rows of one true class.
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct CallCounts {
    pub positive: usize,
    pub negative: usize,
    pub both: usize,
    pub neither: usize,
}

/// Rows of each true class called positive or negative at probability 0.5.
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct ThresholdCounts {
    pub true_positive: usize,
    pub false_negative: usize,
    pub false_positive: usize,
    pub true_negative: usize,
}

/// Evaluation of the fitting procedure by outer cross-validation.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LogisticEnsembleEvaluation {
    pub folds: Vec<EnsembleCvFold>,
    pub mean_fold_auroc: f64,
    pub mean_fold_average_precision: f64,
    /// AUROC of all out-of-fold probabilities together.
    pub pooled_auroc: f64,
    pub pooled_average_precision: f64,
    pub brier_score: f64,
    pub reliability: Vec<ReliabilityBin>,
    pub threshold: ThresholdCounts,
    pub conformal_positive_rows: CallCounts,
    pub conformal_negative_rows: CallCounts,
    /// One prediction per row, in row order.
    pub predictions: Vec<EnsembleCvPrediction>,
}

impl LogisticEnsemble {
    /// Probability, conformal p-values and call for one row of raw features.
    pub fn predict(&self, row: &[f64]) -> Result<EnsemblePrediction, LogisticError> {
        let mut probability = 0.0;
        let (mut positive_count, mut negative_count) = (0usize, 0usize);
        for ((member, positives), negatives) in self
            .members
            .iter()
            .zip(&self.positive_margins)
            .zip(&self.negative_margins)
        {
            let margin = member.margin(row)?;
            probability += self.platt.probability(margin);
            // A held-out positive at or below this margin is at least as unusual for a positive.
            positive_count += positives.partition_point(|&value| value <= margin);
            negative_count += negatives.len() - negatives.partition_point(|&value| value < margin);
        }
        let positives: usize = self.positive_margins.iter().map(Vec::len).sum();
        let negatives: usize = self.negative_margins.iter().map(Vec::len).sum();
        let p_value_positive = (positive_count + 1) as f64 / (positives + 1) as f64;
        let p_value_negative = (negative_count + 1) as f64 / (negatives + 1) as f64;
        let call = match (
            p_value_positive > self.significance,
            p_value_negative > self.significance,
        ) {
            (true, false) => ConformalCall::Positive,
            (false, true) => ConformalCall::Negative,
            (true, true) => ConformalCall::Both,
            (false, false) => ConformalCall::Neither,
        };
        Ok(EnsemblePrediction {
            probability: probability / self.members.len() as f64,
            p_value_positive,
            p_value_negative,
            call,
        })
    }

    /// Number of features the members were fitted on.
    pub fn columns(&self) -> usize {
        self.members[0].columns()
    }

    pub fn selected_c(&self) -> f64 {
        self.selected_c
    }

    pub fn significance(&self) -> f64 {
        self.significance
    }

    pub fn platt(&self) -> PlattScaling {
        self.platt
    }

    pub fn members(&self) -> usize {
        self.members.len()
    }
}

impl TryFrom<EnsembleParts> for LogisticEnsemble {
    type Error = LogisticError;

    fn try_from(parts: EnsembleParts) -> Result<Self, Self::Error> {
        let invalid = |message: &str| Err(LogisticError::Invalid(format!("ensemble: {message}")));
        let Some(first) = parts.members.first() else {
            return invalid("at least one member is required");
        };
        for member in &parts.members {
            member.check()?;
            if member.columns() != first.columns() {
                return invalid("members must have the same number of features");
            }
        }
        if parts.positive_margins.len() != parts.members.len()
            || parts.negative_margins.len() != parts.members.len()
        {
            return invalid("each member needs its held-out margins of both classes");
        }
        let margins = parts.positive_margins.iter().chain(&parts.negative_margins);
        for list in margins {
            if !list.iter().all(|value| value.is_finite())
                || !list.windows(2).all(|pair| pair[0] <= pair[1])
            {
                return invalid("held-out margins must be finite and increasing");
            }
        }
        if parts.positive_margins.iter().all(Vec::is_empty)
            || parts.negative_margins.iter().all(Vec::is_empty)
        {
            return invalid("held-out margins of both classes are required");
        }
        if !(parts.selected_c.is_finite() && parts.selected_c > 0.0)
            || !parts.platt.is_finite()
            || !(parts.significance > 0.0 && parts.significance < 1.0)
        {
            return invalid("C, Platt scaling and significance must be valid");
        }
        Ok(Self {
            selected_c: parts.selected_c,
            members: parts.members,
            platt: parts.platt,
            significance: parts.significance,
            positive_margins: parts.positive_margins,
            negative_margins: parts.negative_margins,
        })
    }
}

/// Evaluate the fitting procedure by grouped outer cross-validation, then fit the final ensemble
/// on every row. `features` is row-major with `labels.len()` rows of `columns` values; rows with
/// the same `groups` value stay in one fold.
pub fn fit_and_evaluate_logistic_ensemble(
    features: &[f64],
    columns: usize,
    labels: &[bool],
    groups: &[usize],
    spec: &LogisticEnsembleSpec,
) -> Result<(LogisticEnsembleEvaluation, LogisticEnsembleFit), LogisticError> {
    validate_spec(spec)?;
    let rows = labels.len();
    if groups.len() != rows || columns == 0 || rows.checked_mul(columns) != Some(features.len()) {
        return Err(LogisticError::Invalid(
            "features, labels and groups must describe the same rows".to_owned(),
        ));
    }
    let data = Data {
        features,
        columns,
        labels,
        groups,
    };
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let all: Vec<usize> = (0..rows).collect();
    let outer = assign_folds(&all, labels, groups, spec.folds, &mut rng)
        .map_err(|message| LogisticError::Invalid(format!("outer folds: {message}")))?;
    let mut folds = Vec::with_capacity(spec.folds);
    let mut predictions = vec![None; rows];
    for (fold, test) in outer.iter().enumerate() {
        let training = complement(&all, test);
        let fit = fit_ensemble(&data, &training, spec, &mut rng)
            .map_err(|error| in_context(error, &format!("outer fold {fold}")))?;
        let mut probabilities = Vec::with_capacity(test.len());
        for &row in test {
            let prediction = fit.ensemble.predict(data.row(row))?;
            probabilities.push(prediction.probability);
            predictions[row] = Some(EnsembleCvPrediction {
                row,
                fold,
                prediction,
            });
        }
        let truth = select(labels, test);
        let (Some(auroc), Some(average_precision)) = (
            area_under_roc(&truth, &probabilities),
            average_precision(&truth, &probabilities),
        ) else {
            return Err(LogisticError::Invalid(format!(
                "outer test fold {fold} lacks a class"
            )));
        };
        folds.push(EnsembleCvFold {
            fold,
            training_rows: training.len(),
            test_rows: test.len(),
            test_positives: truth.iter().filter(|&&label| label).count(),
            heldout_mean_auroc: fit.heldout_mean_auroc,
            selected_c: fit.ensemble.selected_c,
            auroc,
            average_precision,
        });
    }
    let predictions: Vec<EnsembleCvPrediction> = predictions
        .into_iter()
        .map(|prediction| prediction.expect("outer folds partition the rows"))
        .collect();
    let evaluation = summarise(labels, folds, predictions);
    let final_fit = fit_ensemble(&data, &all, spec, &mut rng)
        .map_err(|error| in_context(error, "final ensemble"))?;
    Ok((evaluation, final_fit))
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

struct Data<'a> {
    features: &'a [f64],
    columns: usize,
    labels: &'a [bool],
    groups: &'a [usize],
}

impl Data<'_> {
    fn row(&self, row: usize) -> &[f64] {
        &self.features[row * self.columns..(row + 1) * self.columns]
    }

    fn gather(&self, rows: &[usize]) -> Vec<f64> {
        rows.iter()
            .flat_map(|&row| self.row(row))
            .copied()
            .collect()
    }
}

/// Fit the ensemble on `rows` (increasing).
fn fit_ensemble(
    data: &Data,
    rows: &[usize],
    spec: &LogisticEnsembleSpec,
    rng: &mut ChaCha20Rng,
) -> Result<LogisticEnsembleFit, LogisticError> {
    let folds = assign_folds(rows, data.labels, data.groups, spec.inner_folds, rng)
        .map_err(|message| LogisticError::Invalid(format!("inner folds: {message}")))?;
    // models[fold][c] and margins[fold][c] (held-out margins in the order of the fold's rows).
    let mut models = Vec::with_capacity(folds.len());
    let mut margins = Vec::with_capacity(folds.len());
    for test in &folds {
        let training = complement(rows, test);
        let problem = LogisticProblem::new(
            &data.gather(&training),
            data.columns,
            &select(data.labels, &training),
            spec.class_weight,
        )?;
        let fold_models = problem.fit_path(&spec.c_values)?;
        let fold_margins = fold_models
            .iter()
            .map(|model| {
                test.iter()
                    .map(|&row| model.margin(data.row(row)))
                    .collect::<Result<Vec<f64>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        models.push(fold_models);
        margins.push(fold_margins);
    }
    let mut heldout_mean_auroc = vec![0.0; spec.c_values.len()];
    for (test, fold_margins) in folds.iter().zip(&margins) {
        let truth = select(data.labels, test);
        for (total, scores) in heldout_mean_auroc.iter_mut().zip(fold_margins) {
            *total += area_under_roc(&truth, scores).ok_or_else(|| {
                LogisticError::Invalid("a held-out fold lacks a class".to_owned())
            })?;
        }
    }
    for total in &mut heldout_mean_auroc {
        *total /= folds.len() as f64;
    }
    let best = heldout_mean_auroc
        .iter()
        .enumerate()
        .fold(0, |best, (index, &value)| {
            if value > heldout_mean_auroc[best] {
                index
            } else {
                best
            }
        });
    let members: Vec<LogisticModel> = models
        .into_iter()
        .map(|mut fold_models| fold_models.swap_remove(best))
        .collect();
    let mut pooled_margins = Vec::with_capacity(rows.len());
    let mut pooled_labels = Vec::with_capacity(rows.len());
    let mut positive_margins = Vec::with_capacity(folds.len());
    let mut negative_margins = Vec::with_capacity(folds.len());
    for (test, fold_margins) in folds.iter().zip(&margins) {
        let (mut positives, mut negatives) = (Vec::new(), Vec::new());
        for (&row, &margin) in test.iter().zip(&fold_margins[best]) {
            pooled_margins.push(margin);
            pooled_labels.push(data.labels[row]);
            if data.labels[row] {
                positives.push(margin);
            } else {
                negatives.push(margin);
            }
        }
        positives.sort_by(f64::total_cmp);
        negatives.sort_by(f64::total_cmp);
        positive_margins.push(positives);
        negative_margins.push(negatives);
    }
    let platt = PlattScaling::fit(&pooled_margins, &pooled_labels)?;
    Ok(LogisticEnsembleFit {
        ensemble: LogisticEnsemble {
            selected_c: spec.c_values[best],
            members,
            platt,
            significance: spec.significance,
            positive_margins,
            negative_margins,
        },
        heldout_mean_auroc,
    })
}

fn summarise(
    labels: &[bool],
    folds: Vec<EnsembleCvFold>,
    predictions: Vec<EnsembleCvPrediction>,
) -> LogisticEnsembleEvaluation {
    let probabilities: Vec<f64> = predictions
        .iter()
        .map(|p| p.prediction.probability)
        .collect();
    let mut threshold = ThresholdCounts::default();
    let mut conformal_positive_rows = CallCounts::default();
    let mut conformal_negative_rows = CallCounts::default();
    for (&label, prediction) in labels.iter().zip(&predictions) {
        let called_positive = prediction.prediction.probability >= 0.5;
        match (label, called_positive) {
            (true, true) => threshold.true_positive += 1,
            (true, false) => threshold.false_negative += 1,
            (false, true) => threshold.false_positive += 1,
            (false, false) => threshold.true_negative += 1,
        }
        let counts = if label {
            &mut conformal_positive_rows
        } else {
            &mut conformal_negative_rows
        };
        match prediction.prediction.call {
            ConformalCall::Positive => counts.positive += 1,
            ConformalCall::Negative => counts.negative += 1,
            ConformalCall::Both => counts.both += 1,
            ConformalCall::Neither => counts.neither += 1,
        }
    }
    let mean = |values: Vec<f64>| values.iter().sum::<f64>() / values.len() as f64;
    LogisticEnsembleEvaluation {
        mean_fold_auroc: mean(folds.iter().map(|f| f.auroc).collect()),
        mean_fold_average_precision: mean(folds.iter().map(|f| f.average_precision).collect()),
        pooled_auroc: area_under_roc(labels, &probabilities).expect("both classes are present"),
        pooled_average_precision: average_precision(labels, &probabilities)
            .expect("both classes are present"),
        brier_score: brier_score(labels, &probabilities),
        reliability: reliability(labels, &probabilities),
        threshold,
        conformal_positive_rows,
        conformal_negative_rows,
        folds,
        predictions,
    }
}

fn in_context(error: LogisticError, context: &str) -> LogisticError {
    match error {
        LogisticError::Invalid(message) => LogisticError::Invalid(format!("{context}: {message}")),
        LogisticError::Resource(message) => {
            LogisticError::Resource(format!("{context}: {message}"))
        }
        LogisticError::Numerical(message) => {
            LogisticError::Numerical(format!("{context}: {message}"))
        }
    }
}

fn validate_spec(spec: &LogisticEnsembleSpec) -> Result<(), LogisticError> {
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
    if !(spec.significance > 0.0 && spec.significance < 1.0) {
        return invalid("significance must lie strictly between 0 and 1".to_owned());
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

/// The rows of `rows` (increasing) that are not in `excluded` (increasing).
fn complement(rows: &[usize], excluded: &[usize]) -> Vec<usize> {
    rows.iter()
        .copied()
        .filter(|row| excluded.binary_search(row).is_err())
        .collect()
}

fn select<T: Copy>(values: &[T], rows: &[usize]) -> Vec<T> {
    rows.iter().map(|&row| values[row]).collect()
}
