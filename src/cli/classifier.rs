//! `marklab classifier fit` and `predict`: a calibrated logistic regression ensemble with
//! conformal calls, predicting a binary label from a feature table of any modality.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use marklab_prediction::{
    fit_and_evaluate_logistic_ensemble, CallCounts, ConformalCall, EnsembleCvFold,
    LogisticEnsemble, LogisticEnsembleSpec, LogisticError, ReliabilityBin,
};
use serde::{Deserialize, Serialize};

use super::feature_table::{read_feature_table, write_new_json, FeatureTable};
use crate::{MarklabError, Result};

const MODEL_FORMAT: &str = "marklab.classifier";
const MAXIMUM_MODEL_BYTES: u64 = 1024 * 1024 * 1024;

/// Arguments of `marklab classifier fit`.
#[derive(clap::Args, Debug)]
pub struct FitArgs {
    /// Feature table (Parquet) with one row per unit.
    #[arg(long)]
    features: PathBuf,
    /// CSV table with a header row holding each row's ID and label, and optionally a subset
    /// column and a group.
    #[arg(long)]
    labels: PathBuf,
    /// JSON configuration naming the columns, label values and fitting settings.
    #[arg(long)]
    config: PathBuf,
    /// File to write the fitted model to; it must not already exist.
    #[arg(long)]
    model: PathBuf,
    /// File to write the evaluation report to; it must not already exist.
    #[arg(long)]
    report: PathBuf,
}

/// Arguments of `marklab classifier predict`.
#[derive(clap::Args, Debug)]
pub struct PredictArgs {
    /// Model written by `marklab classifier fit`.
    #[arg(long)]
    model: PathBuf,
    /// Feature table (Parquet) made the same way as the model's training table.
    #[arg(long)]
    features: PathBuf,
    /// File to write the predictions to; it must not already exist.
    #[arg(long)]
    out: PathBuf,
}

/// Which rows are modelled and how their labels become classes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Target {
    /// Label-table column matching the feature table's `id`.
    id_column: String,
    /// Column of the unit kept within one fold, such as the patient; each row is its own group
    /// when absent.
    #[serde(default)]
    group_column: Option<String>,
    /// Model only rows whose `column` equals `value`, such as one species.
    #[serde(default)]
    subset: Option<Subset>,
    label_column: String,
    /// Label values of the positive class, such as `["R", "I"]`.
    positive: Vec<String>,
    /// Label values of the negative class, such as `["S"]`.
    negative: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Subset {
    column: String,
    value: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FitConfig {
    target: Target,
    model: LogisticEnsembleSpec,
}

/// What a model accepts: tables of the same modality, unit, provenance and columns.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FeatureSchema {
    modality: String,
    unit: String,
    provenance: serde_json::Value,
    columns: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ClassifierModel {
    format: String,
    version: u32,
    features: FeatureSchema,
    target: Target,
    training: LabelSummary,
    settings: LogisticEnsembleSpec,
    ensemble: LogisticEnsemble,
}

struct Labelled {
    id: String,
    group: Option<String>,
    positive: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LabelSummary {
    rows: usize,
    outside_subset: usize,
    missing_label: usize,
    labelled: usize,
    positive: usize,
    negative: usize,
    groups: usize,
    unlabelled_feature_rows: usize,
}

/// A conformal prediction.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Call {
    Positive,
    Negative,
    /// Both classes remain plausible: abstain and decide another way.
    Uncertain,
    /// Neither class is plausible: the row is unlike the training rows.
    Neither,
}

impl From<ConformalCall> for Call {
    fn from(call: ConformalCall) -> Self {
        match call {
            ConformalCall::Positive => Self::Positive,
            ConformalCall::Negative => Self::Negative,
            ConformalCall::Both => Self::Uncertain,
            ConformalCall::Neither => Self::Neither,
        }
    }
}

#[derive(Serialize)]
struct CallsOfClass {
    called_positive: usize,
    called_negative: usize,
    uncertain: usize,
    neither: usize,
}

impl From<&CallCounts> for CallsOfClass {
    fn from(counts: &CallCounts) -> Self {
        Self {
            called_positive: counts.positive,
            called_negative: counts.negative,
            uncertain: counts.both,
            neither: counts.neither,
        }
    }
}

#[derive(Serialize)]
struct ErrorRates {
    accuracy: Option<f64>,
    false_negatives: usize,
    false_negative_rate: f64,
    false_positives: usize,
    false_positive_rate: f64,
}

#[derive(Serialize)]
struct ConformalSummary {
    significance: f64,
    positive_rows: CallsOfClass,
    negative_rows: CallsOfClass,
    #[serde(flatten)]
    errors: ErrorRates,
    abstention_rate: f64,
    positive_coverage: f64,
    negative_coverage: f64,
}

#[derive(Serialize)]
struct RowPrediction<'a> {
    id: &'a str,
    probability_positive: f64,
    p_value_positive: f64,
    p_value_negative: f64,
    call: Call,
}

#[derive(Serialize)]
struct EvaluatedPrediction<'a> {
    #[serde(flatten)]
    prediction: RowPrediction<'a>,
    group: Option<&'a str>,
    fold: usize,
    positive: bool,
}

#[derive(Serialize)]
struct Evaluation<'a> {
    folds: Vec<EnsembleCvFold>,
    mean_fold_auroc: f64,
    mean_fold_average_precision: f64,
    pooled_auroc: f64,
    pooled_average_precision: f64,
    brier_score: f64,
    reliability: Vec<ReliabilityBin>,
    /// Calls at calibrated probability 0.5, without abstention.
    threshold_0_5: ErrorRates,
    conformal: ConformalSummary,
    predictions: Vec<EvaluatedPrediction<'a>>,
}

#[derive(Serialize)]
struct FinalFit {
    selected_c: f64,
    heldout_mean_auroc: Vec<f64>,
    members: usize,
}

#[derive(Serialize)]
struct FitReport<'a> {
    config: &'a FitConfig,
    features: &'a FeatureSchema,
    labels: &'a LabelSummary,
    evaluation: Evaluation<'a>,
    final_ensemble: FinalFit,
}

#[derive(Serialize)]
struct PredictOutput<'a> {
    modality: &'a str,
    unit: &'a str,
    label_column: &'a str,
    subset: Option<&'a Subset>,
    significance: f64,
    predictions: Vec<RowPrediction<'a>>,
}

pub(super) fn run_fit(args: FitArgs) -> Result<()> {
    let bytes = super::simulate::read_input(&args.config, "classifier configuration")?;
    let config: FitConfig = serde_json::from_slice(&bytes)?;
    validate_target(&config.target)?;
    let table = read_feature_table(&args.features)?;
    let labels_bytes = super::simulate::read_input(&args.labels, "label table")?;
    let (labelled, mut summary) = read_labels(&labels_bytes, &config.target)?;
    let row_of: HashMap<&str, usize> = table
        .ids
        .iter()
        .enumerate()
        .map(|(row, id)| (id.as_str(), row))
        .collect();
    let missing: Vec<&str> = labelled
        .iter()
        .filter(|row| !row_of.contains_key(row.id.as_str()))
        .map(|row| row.id.as_str())
        .collect();
    if !missing.is_empty() {
        bail!(
            "{} labelled IDs are not in the feature table, such as `{}`",
            missing.len(),
            missing[0]
        );
    }
    let width = table.columns.len();
    let mut features = Vec::with_capacity(labelled.len() * width);
    for row in &labelled {
        let start = row_of[row.id.as_str()] * width;
        features.extend_from_slice(&table.values[start..start + width]);
    }
    summary.unlabelled_feature_rows = table.ids.len() - labelled.len();
    let classes: Vec<bool> = labelled.iter().map(|row| row.positive).collect();
    let mut group_index = BTreeMap::new();
    let groups: Vec<usize> = labelled
        .iter()
        .map(|row| {
            let key = row.group.as_deref().unwrap_or(&row.id);
            let next = group_index.len();
            *group_index.entry(key).or_insert(next)
        })
        .collect();
    summary.groups = group_index.len();
    let (evaluation, final_fit) =
        fit_and_evaluate_logistic_ensemble(&features, width, &classes, &groups, &config.model)
            .map_err(logistic_error)?;

    let schema = schema_of(&table);
    let model = ClassifierModel {
        format: MODEL_FORMAT.to_owned(),
        version: 1,
        features: schema.clone(),
        target: config.target.clone(),
        training: summary.clone(),
        settings: config.model.clone(),
        ensemble: final_fit.ensemble,
    };
    let positive = summary.positive;
    let negative = summary.negative;
    let conformal_positive = &evaluation.conformal_positive_rows;
    let conformal_negative = &evaluation.conformal_negative_rows;
    let threshold = &evaluation.threshold;
    let report = FitReport {
        config: &config,
        features: &schema,
        labels: &summary,
        evaluation: Evaluation {
            mean_fold_auroc: evaluation.mean_fold_auroc,
            mean_fold_average_precision: evaluation.mean_fold_average_precision,
            pooled_auroc: evaluation.pooled_auroc,
            pooled_average_precision: evaluation.pooled_average_precision,
            brier_score: evaluation.brier_score,
            reliability: evaluation.reliability.clone(),
            threshold_0_5: error_rates(
                Some(threshold.true_positive + threshold.true_negative),
                threshold.false_negative,
                threshold.false_positive,
                positive,
                negative,
            ),
            conformal: ConformalSummary {
                significance: config.model.significance,
                positive_rows: conformal_positive.into(),
                negative_rows: conformal_negative.into(),
                errors: error_rates(
                    None,
                    conformal_positive.negative,
                    conformal_negative.positive,
                    positive,
                    negative,
                ),
                abstention_rate: (conformal_positive.both
                    + conformal_positive.neither
                    + conformal_negative.both
                    + conformal_negative.neither) as f64
                    / labelled.len() as f64,
                positive_coverage: (conformal_positive.positive + conformal_positive.both) as f64
                    / positive as f64,
                negative_coverage: (conformal_negative.negative + conformal_negative.both) as f64
                    / negative as f64,
            },
            predictions: evaluation
                .predictions
                .iter()
                .map(|prediction| {
                    let row = &labelled[prediction.row];
                    EvaluatedPrediction {
                        prediction: row_prediction(&row.id, &prediction.prediction),
                        group: row.group.as_deref(),
                        fold: prediction.fold,
                        positive: row.positive,
                    }
                })
                .collect(),
            folds: evaluation.folds.clone(),
        },
        final_ensemble: FinalFit {
            selected_c: model.ensemble.selected_c(),
            heldout_mean_auroc: final_fit.heldout_mean_auroc,
            members: model.ensemble.members(),
        },
    };
    write_new_json(&args.model, &model)?;
    write_new_json(&args.report, &report)
}

pub(super) fn run_predict(args: PredictArgs) -> Result<()> {
    let model = read_model(&args.model)?;
    let table = read_feature_table(&args.features)?;
    let schema = schema_of(&table);
    let expected = &model.features;
    let mismatch = if schema.modality != expected.modality || schema.unit != expected.unit {
        Some("modality or unit")
    } else if schema.provenance != expected.provenance {
        Some("provenance (how the features were made)")
    } else if schema.columns != expected.columns {
        Some("feature columns")
    } else {
        None
    };
    if let Some(what) = mismatch {
        bail!(
            "feature table {}: its {what} differs from the model's training table",
            args.features.display()
        );
    }
    let width = table.columns.len();
    let predictions = table
        .ids
        .iter()
        .zip(table.values.chunks_exact(width))
        .map(|(id, row)| {
            let prediction = model.ensemble.predict(row).map_err(logistic_error)?;
            Ok(row_prediction(id, &prediction))
        })
        .collect::<Result<Vec<_>>>()?;
    let output = PredictOutput {
        modality: &expected.modality,
        unit: &expected.unit,
        label_column: &model.target.label_column,
        subset: model.target.subset.as_ref(),
        significance: model.ensemble.significance(),
        predictions,
    };
    write_new_json(&args.out, &output)
}

fn row_prediction<'a>(
    id: &'a str,
    prediction: &marklab_prediction::EnsemblePrediction,
) -> RowPrediction<'a> {
    RowPrediction {
        id,
        probability_positive: prediction.probability,
        p_value_positive: prediction.p_value_positive,
        p_value_negative: prediction.p_value_negative,
        call: prediction.call.into(),
    }
}

/// False negatives call a positive row negative; false positives call a negative row positive.
/// Rates are per positive and per negative row.
fn error_rates(
    agreeing: Option<usize>,
    false_negatives: usize,
    false_positives: usize,
    positive: usize,
    negative: usize,
) -> ErrorRates {
    ErrorRates {
        accuracy: agreeing.map(|count| count as f64 / (positive + negative) as f64),
        false_negatives,
        false_negative_rate: false_negatives as f64 / positive as f64,
        false_positives,
        false_positive_rate: false_positives as f64 / negative as f64,
    }
}

fn schema_of(table: &FeatureTable) -> FeatureSchema {
    FeatureSchema {
        modality: table.meta.modality.clone(),
        unit: table.meta.unit.clone(),
        provenance: table.meta.provenance.clone(),
        columns: table.columns.clone(),
    }
}

fn read_model(path: &Path) -> Result<ClassifierModel> {
    let io_error = |source| MarklabError::io(path, source);
    if !fs::metadata(path).map_err(io_error)?.is_file() {
        bail!("{}: a model must be a regular file", path.display());
    }
    let file = File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > MAXIMUM_MODEL_BYTES {
        bail!("{}: a model may be at most 1 GiB", path.display());
    }
    let reader = BufReader::new(file.take(MAXIMUM_MODEL_BYTES));
    let model: ClassifierModel = serde_json::from_reader(reader)
        .map_err(|error| MarklabError::Validation(format!("{}: {error}", path.display())))?;
    if model.format != MODEL_FORMAT || model.version != 1 {
        bail!("{}: not a version 1 `{MODEL_FORMAT}` file", path.display());
    }
    if model.features.columns.len() != model.ensemble.columns() {
        bail!(
            "{}: the model's feature columns and ensemble disagree",
            path.display()
        );
    }
    Ok(model)
}

fn logistic_error(error: LogisticError) -> MarklabError {
    MarklabError::Validation(error.to_string())
}

fn validate_target(target: &Target) -> Result<()> {
    if target
        .subset
        .as_ref()
        .is_some_and(|subset| subset.value.trim().is_empty())
    {
        bail!("classifier configuration: the subset value must not be empty");
    }
    let mut values = target.positive.iter().chain(&target.negative);
    if target.positive.is_empty()
        || target.negative.is_empty()
        || values.any(|value| value.trim().is_empty() || value.trim() != value)
    {
        bail!(
            "classifier configuration: positive and negative must each list at least one label \
             value without surrounding spaces"
        );
    }
    if target
        .positive
        .iter()
        .any(|value| target.negative.contains(value))
    {
        bail!("classifier configuration: a label value cannot be both positive and negative");
    }
    Ok(())
}

fn read_labels(table: &[u8], target: &Target) -> Result<(Vec<Labelled>, LabelSummary)> {
    let failed = |error: csv::Error| MarklabError::Validation(format!("label table: {error}"));
    let mut reader = csv::ReaderBuilder::new().from_reader(table);
    let headers = reader.headers().map_err(failed)?.clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| MarklabError::Validation(format!("label table has no column `{name}`")))
    };
    let id_column = column(&target.id_column)?;
    let label_column = column(&target.label_column)?;
    let subset_column = target
        .subset
        .as_ref()
        .map(|subset| column(&subset.column))
        .transpose()?;
    let group_column = target.group_column.as_deref().map(column).transpose()?;
    let mut summary = LabelSummary {
        rows: 0,
        outside_subset: 0,
        missing_label: 0,
        labelled: 0,
        positive: 0,
        negative: 0,
        groups: 0,
        unlabelled_feature_rows: 0,
    };
    let mut labelled = Vec::new();
    let mut seen = BTreeSet::new();
    for record in reader.records() {
        let record = record.map_err(failed)?;
        summary.rows += 1;
        let line = record.position().map_or(0, |position| position.line());
        if let (Some(index), Some(subset)) = (subset_column, target.subset.as_ref()) {
            if record[index].trim() != subset.value {
                summary.outside_subset += 1;
                continue;
            }
        }
        let label = record[label_column].trim();
        if label.is_empty() {
            summary.missing_label += 1;
            continue;
        }
        let positive = if target.positive.iter().any(|value| value == label) {
            true
        } else if target.negative.iter().any(|value| value == label) {
            false
        } else {
            bail!(
                "label table line {line}: `{}` value `{label}` is listed as neither positive \
                 nor negative",
                target.label_column
            );
        };
        let id = record[id_column].trim();
        if id.is_empty() {
            bail!("label table line {line}: the ID is empty");
        }
        if !seen.insert(id.to_owned()) {
            bail!("label table line {line}: ID `{id}` appears more than once");
        }
        let group = match group_column {
            Some(index) if record[index].trim().is_empty() => {
                bail!("label table line {line}: ID `{id}` has no group");
            }
            Some(index) => Some(record[index].trim().to_owned()),
            None => None,
        };
        if positive {
            summary.positive += 1;
        } else {
            summary.negative += 1;
        }
        labelled.push(Labelled {
            id: id.to_owned(),
            group,
            positive,
        });
    }
    summary.labelled = labelled.len();
    if labelled.is_empty() {
        bail!("label table: no row has a listed label value");
    }
    Ok((labelled, summary))
}
