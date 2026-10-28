//! `marklab microbio fit-ast` and `predict-ast`: a calibrated logistic regression ensemble
//! predicting one antibiotic's resistance from a feature table of any modality.

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

use super::feature_table::{read_feature_table, FeatureTable, Unit};
use crate::{MarklabError, Result};

const MODEL_FORMAT: &str = "marklab.ast-model";
const MAXIMUM_MODEL_BYTES: u64 = 1024 * 1024 * 1024;

/// Arguments of `marklab microbio fit-ast`.
#[derive(clap::Args, Debug)]
pub struct FitAstArgs {
    /// Feature table (Parquet) with one row per isolate or specimen.
    #[arg(long)]
    features: PathBuf,
    /// CSV table with a header row holding each row's ID, susceptibility category and
    /// optionally species and group.
    #[arg(long)]
    labels: PathBuf,
    /// JSON configuration naming the columns, categories and fitting settings.
    #[arg(long)]
    config: PathBuf,
    /// File to write the fitted model to; it must not already exist.
    #[arg(long)]
    model: PathBuf,
    /// File to write the evaluation report to; it must not already exist.
    #[arg(long)]
    report: PathBuf,
}

/// Arguments of `marklab microbio predict-ast`.
#[derive(clap::Args, Debug)]
pub struct PredictAstArgs {
    /// Model written by `marklab microbio fit-ast`.
    #[arg(long)]
    model: PathBuf,
    /// Feature table (Parquet) made the same way as the model's training table.
    #[arg(long)]
    features: PathBuf,
    /// File to write the predictions to; it must not already exist.
    #[arg(long)]
    out: PathBuf,
}

/// Which rows are modelled and how their categories become classes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Target {
    /// Label-table column matching the feature table's `id`.
    id_column: String,
    /// Column of the unit kept within one fold, such as the patient; each row is its own group
    /// when absent.
    #[serde(default)]
    group_column: Option<String>,
    /// Restrict to rows whose `species_column` equals `species`; both or neither.
    #[serde(default)]
    species_column: Option<String>,
    #[serde(default)]
    species: Option<String>,
    antibiotic_column: String,
    resistant: Vec<String>,
    susceptible: Vec<String>,
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
    unit: Unit,
    provenance: serde_json::Value,
    columns: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AstModel {
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
    resistant: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LabelSummary {
    rows: usize,
    other_species: usize,
    missing_category: usize,
    labelled: usize,
    resistant: usize,
    susceptible: usize,
    groups: usize,
    unlabelled_feature_rows: usize,
}

/// A conformal prediction in AST terms.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Call {
    Resistant,
    Susceptible,
    /// Both categories remain plausible: confirm with conventional AST.
    Uncertain,
    /// Neither category is plausible: the spectrum is unlike the training isolates.
    Neither,
}

impl From<ConformalCall> for Call {
    fn from(call: ConformalCall) -> Self {
        match call {
            ConformalCall::Positive => Self::Resistant,
            ConformalCall::Negative => Self::Susceptible,
            ConformalCall::Both => Self::Uncertain,
            ConformalCall::Neither => Self::Neither,
        }
    }
}

#[derive(Serialize)]
struct CallsOfClass {
    called_resistant: usize,
    called_susceptible: usize,
    uncertain: usize,
    neither: usize,
}

impl From<&CallCounts> for CallsOfClass {
    fn from(counts: &CallCounts) -> Self {
        Self {
            called_resistant: counts.positive,
            called_susceptible: counts.negative,
            uncertain: counts.both,
            neither: counts.neither,
        }
    }
}

#[derive(Serialize)]
struct ErrorRates {
    categorical_agreement: Option<f64>,
    very_major_errors: usize,
    very_major_error_rate: f64,
    major_errors: usize,
    major_error_rate: f64,
}

#[derive(Serialize)]
struct ConformalSummary {
    significance: f64,
    resistant_isolates: CallsOfClass,
    susceptible_isolates: CallsOfClass,
    #[serde(flatten)]
    errors: ErrorRates,
    abstention_rate: f64,
    resistant_coverage: f64,
    susceptible_coverage: f64,
}

#[derive(Serialize)]
struct RowPrediction<'a> {
    id: &'a str,
    probability_resistant: f64,
    p_value_resistant: f64,
    p_value_susceptible: f64,
    call: Call,
}

#[derive(Serialize)]
struct EvaluatedPrediction<'a> {
    #[serde(flatten)]
    prediction: RowPrediction<'a>,
    group: Option<&'a str>,
    fold: usize,
    resistant: bool,
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
    unit: Unit,
    antibiotic_column: &'a str,
    species: Option<&'a str>,
    significance: f64,
    predictions: Vec<RowPrediction<'a>>,
}

pub(in crate::cli) fn run_fit_ast(args: FitAstArgs) -> Result<()> {
    let bytes = super::super::simulate::read_input(&args.config, "AST configuration")?;
    let config: FitConfig = serde_json::from_slice(&bytes)?;
    validate_target(&config.target)?;
    let table = read_feature_table(&args.features)?;
    let labels_bytes = super::super::simulate::read_input(&args.labels, "AST label table")?;
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
    let classes: Vec<bool> = labelled.iter().map(|row| row.resistant).collect();
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
    let model = AstModel {
        format: MODEL_FORMAT.to_owned(),
        version: 1,
        features: schema.clone(),
        target: config.target.clone(),
        training: summary.clone(),
        settings: config.model.clone(),
        ensemble: final_fit.ensemble,
    };
    let resistant = summary.resistant;
    let susceptible = summary.susceptible;
    let conformal_resistant = &evaluation.conformal_positive_rows;
    let conformal_susceptible = &evaluation.conformal_negative_rows;
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
                resistant,
                susceptible,
            ),
            conformal: ConformalSummary {
                significance: config.model.significance,
                resistant_isolates: conformal_resistant.into(),
                susceptible_isolates: conformal_susceptible.into(),
                errors: error_rates(
                    None,
                    conformal_resistant.negative,
                    conformal_susceptible.positive,
                    resistant,
                    susceptible,
                ),
                abstention_rate: (conformal_resistant.both
                    + conformal_resistant.neither
                    + conformal_susceptible.both
                    + conformal_susceptible.neither) as f64
                    / labelled.len() as f64,
                resistant_coverage: (conformal_resistant.positive + conformal_resistant.both)
                    as f64
                    / resistant as f64,
                susceptible_coverage: (conformal_susceptible.negative + conformal_susceptible.both)
                    as f64
                    / susceptible as f64,
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
                        resistant: row.resistant,
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
    super::write_new_json(&args.model, &model)?;
    super::write_new_json(&args.report, &report)
}

pub(in crate::cli) fn run_predict_ast(args: PredictAstArgs) -> Result<()> {
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
        unit: expected.unit,
        antibiotic_column: &model.target.antibiotic_column,
        species: model.target.species.as_deref(),
        significance: model.ensemble.significance(),
        predictions,
    };
    super::write_new_json(&args.out, &output)
}

fn row_prediction<'a>(
    id: &'a str,
    prediction: &marklab_prediction::EnsemblePrediction,
) -> RowPrediction<'a> {
    RowPrediction {
        id,
        probability_resistant: prediction.probability,
        p_value_resistant: prediction.p_value_positive,
        p_value_susceptible: prediction.p_value_negative,
        call: prediction.call.into(),
    }
}

/// Very major errors call a resistant isolate susceptible; major errors call a susceptible
/// isolate resistant. Rates are per resistant and per susceptible isolate.
fn error_rates(
    agreeing: Option<usize>,
    very_major_errors: usize,
    major_errors: usize,
    resistant: usize,
    susceptible: usize,
) -> ErrorRates {
    ErrorRates {
        categorical_agreement: agreeing
            .map(|count| count as f64 / (resistant + susceptible) as f64),
        very_major_errors,
        very_major_error_rate: very_major_errors as f64 / resistant as f64,
        major_errors,
        major_error_rate: major_errors as f64 / susceptible as f64,
    }
}

fn schema_of(table: &FeatureTable) -> FeatureSchema {
    FeatureSchema {
        modality: table.meta.modality.clone(),
        unit: table.meta.unit,
        provenance: table.meta.provenance.clone(),
        columns: table.columns.clone(),
    }
}

fn read_model(path: &Path) -> Result<AstModel> {
    let io_error = |source| MarklabError::io(path, source);
    if !fs::metadata(path).map_err(io_error)?.is_file() {
        bail!("{}: a model must be a regular file", path.display());
    }
    let file = File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > MAXIMUM_MODEL_BYTES {
        bail!("{}: a model may be at most 1 GiB", path.display());
    }
    let reader = BufReader::new(file.take(MAXIMUM_MODEL_BYTES));
    let model: AstModel = serde_json::from_reader(reader)
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
    if target.species_column.is_some() != target.species.is_some() {
        bail!("AST configuration: give both species_column and species, or neither");
    }
    if target
        .species
        .as_deref()
        .is_some_and(|species| species.trim().is_empty())
    {
        bail!("AST configuration: species must not be empty");
    }
    let mut categories = target.resistant.iter().chain(&target.susceptible);
    if target.resistant.is_empty()
        || target.susceptible.is_empty()
        || categories.any(|value| value.trim().is_empty() || value.trim() != value)
    {
        bail!(
            "AST configuration: resistant and susceptible must each list at least one category \
             without surrounding spaces"
        );
    }
    if target
        .resistant
        .iter()
        .any(|value| target.susceptible.contains(value))
    {
        bail!("AST configuration: a category cannot be both resistant and susceptible");
    }
    Ok(())
}

fn read_labels(table: &[u8], target: &Target) -> Result<(Vec<Labelled>, LabelSummary)> {
    let failed = |error: csv::Error| MarklabError::Validation(format!("AST label table: {error}"));
    let mut reader = csv::ReaderBuilder::new().from_reader(table);
    let headers = reader.headers().map_err(failed)?.clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| {
                MarklabError::Validation(format!("AST label table has no column `{name}`"))
            })
    };
    let id_column = column(&target.id_column)?;
    let antibiotic_column = column(&target.antibiotic_column)?;
    let species_column = target.species_column.as_deref().map(column).transpose()?;
    let group_column = target.group_column.as_deref().map(column).transpose()?;
    let mut summary = LabelSummary {
        rows: 0,
        other_species: 0,
        missing_category: 0,
        labelled: 0,
        resistant: 0,
        susceptible: 0,
        groups: 0,
        unlabelled_feature_rows: 0,
    };
    let mut labelled = Vec::new();
    let mut seen = BTreeSet::new();
    for record in reader.records() {
        let record = record.map_err(failed)?;
        summary.rows += 1;
        let line = record.position().map_or(0, |position| position.line());
        if let (Some(index), Some(species)) = (species_column, target.species.as_deref()) {
            if record[index].trim() != species {
                summary.other_species += 1;
                continue;
            }
        }
        let category = record[antibiotic_column].trim();
        if category.is_empty() {
            summary.missing_category += 1;
            continue;
        }
        let resistant = if target.resistant.iter().any(|value| value == category) {
            true
        } else if target.susceptible.iter().any(|value| value == category) {
            false
        } else {
            bail!(
                "AST label table line {line}: `{}` value `{category}` is listed as neither \
                 resistant nor susceptible",
                target.antibiotic_column
            );
        };
        let id = record[id_column].trim();
        if id.is_empty() {
            bail!("AST label table line {line}: the ID is empty");
        }
        if !seen.insert(id.to_owned()) {
            bail!("AST label table line {line}: ID `{id}` appears more than once");
        }
        let group = match group_column {
            Some(index) if record[index].trim().is_empty() => {
                bail!("AST label table line {line}: ID `{id}` has no group");
            }
            Some(index) => Some(record[index].trim().to_owned()),
            None => None,
        };
        if resistant {
            summary.resistant += 1;
        } else {
            summary.susceptible += 1;
        }
        labelled.push(Labelled {
            id: id.to_owned(),
            group,
            resistant,
        });
    }
    summary.labelled = labelled.len();
    if labelled.is_empty() {
        bail!("AST label table: no row has a listed category");
    }
    Ok((labelled, summary))
}
