//! `marklab microbio cross-validate-ast`: how well binned spectra predict one antibiotic's
//! susceptibility category within one species.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use marklab_prediction::{cross_validate_logistic, LogisticCvSpec, MAXIMUM_LOGISTIC_VALUES};
use serde::{Deserialize, Serialize};

use crate::{MarklabError, Result};

const MAXIMUM_ISOLATE_ID_BYTES: usize = 128;
const MAXIMUM_SPECTRUM_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Arguments of `marklab microbio cross-validate-ast`.
#[derive(clap::Args, Debug)]
pub struct CrossValidateAstArgs {
    /// CSV table with a header row and one row per isolate, holding its ID, species and
    /// susceptibility categories.
    #[arg(long)]
    labels: PathBuf,
    /// Directory of `marklab microbio preprocess` results with `binning`, one
    /// `<isolate ID>.json` per isolate.
    #[arg(long)]
    spectra: PathBuf,
    /// JSON configuration naming the columns, species, categories and cross-validation.
    #[arg(long)]
    config: PathBuf,
    /// File to write the result to; it must not already exist.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AstConfig {
    isolate_column: String,
    /// Column of the unit kept together in one fold, such as the patient. Each isolate is its own
    /// group when absent.
    #[serde(default)]
    group_column: Option<String>,
    species_column: String,
    species: String,
    antibiotic_column: String,
    resistant: Vec<String>,
    susceptible: Vec<String>,
    model: LogisticCvSpec,
}

struct Isolate {
    id: String,
    group: Option<String>,
    resistant: bool,
}

#[derive(Serialize)]
struct LabelSummary {
    rows: usize,
    other_species: usize,
    missing_category: usize,
    isolates: usize,
    resistant: usize,
    susceptible: usize,
    groups: usize,
}

#[derive(Deserialize)]
struct PreprocessResult {
    binned: Option<Binned>,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct Layout {
    min_mz: f64,
    max_mz: f64,
    bins: usize,
}

#[derive(Deserialize)]
struct Binned {
    #[serde(flatten)]
    layout: Layout,
    intensity: Vec<f64>,
}

#[derive(Serialize)]
struct IsolatePrediction<'a> {
    isolate: &'a str,
    group: Option<&'a str>,
    fold: usize,
    resistant: bool,
    probability_resistant: f64,
}

#[derive(Serialize)]
struct AstOutput<'a> {
    config: &'a AstConfig,
    labels: LabelSummary,
    features: Layout,
    folds: Vec<marklab_prediction::LogisticCvFold>,
    mean_fold_auroc: f64,
    mean_fold_average_precision: f64,
    pooled_auroc: f64,
    pooled_average_precision: f64,
    predictions: Vec<IsolatePrediction<'a>>,
}

pub(in crate::cli) fn run_cross_validate_ast(args: CrossValidateAstArgs) -> Result<()> {
    let bytes = super::super::simulate::read_input(&args.config, "AST configuration")?;
    let config: AstConfig = serde_json::from_slice(&bytes)?;
    validate_config(&config)?;
    let table = super::super::simulate::read_input(&args.labels, "AST label table")?;
    let (isolates, mut summary) = read_labels(&table, &config)?;
    let (layout, features) = read_features(&args.spectra, &isolates)?;
    let labels: Vec<bool> = isolates.iter().map(|isolate| isolate.resistant).collect();
    let mut group_index = BTreeMap::new();
    let groups: Vec<usize> = isolates
        .iter()
        .map(|isolate| {
            let key = isolate.group.as_deref().unwrap_or(&isolate.id);
            let next = group_index.len();
            *group_index.entry(key).or_insert(next)
        })
        .collect();
    summary.groups = group_index.len();
    let result = cross_validate_logistic(&features, layout.bins, &labels, &groups, &config.model)
        .map_err(|error| MarklabError::Validation(error.to_string()))?;
    let predictions = result
        .predictions
        .iter()
        .map(|prediction| {
            let isolate = &isolates[prediction.row];
            IsolatePrediction {
                isolate: &isolate.id,
                group: isolate.group.as_deref(),
                fold: prediction.fold,
                resistant: isolate.resistant,
                probability_resistant: prediction.probability,
            }
        })
        .collect();
    let output = AstOutput {
        config: &config,
        labels: summary,
        features: layout,
        folds: result.folds,
        mean_fold_auroc: result.mean_fold_auroc,
        mean_fold_average_precision: result.mean_fold_average_precision,
        pooled_auroc: result.pooled_auroc,
        pooled_average_precision: result.pooled_average_precision,
        predictions,
    };
    super::write_new_json(&args.out, &output)
}

fn validate_config(config: &AstConfig) -> Result<()> {
    let invalid = |message: &str| {
        Err(MarklabError::Validation(format!(
            "AST configuration: {message}"
        )))
    };
    if config.species.trim().is_empty() {
        return invalid("species must not be empty");
    }
    let mut categories = config.resistant.iter().chain(&config.susceptible);
    if config.resistant.is_empty()
        || config.susceptible.is_empty()
        || categories.any(|value| value.trim().is_empty() || value.trim() != value)
    {
        return invalid(
            "resistant and susceptible must each list at least one category without surrounding spaces",
        );
    }
    if config
        .resistant
        .iter()
        .any(|value| config.susceptible.contains(value))
    {
        return invalid("a category cannot be both resistant and susceptible");
    }
    Ok(())
}

fn read_labels(table: &[u8], config: &AstConfig) -> Result<(Vec<Isolate>, LabelSummary)> {
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
    let isolate_column = column(&config.isolate_column)?;
    let species_column = column(&config.species_column)?;
    let antibiotic_column = column(&config.antibiotic_column)?;
    let group_column = config.group_column.as_deref().map(column).transpose()?;
    let mut summary = LabelSummary {
        rows: 0,
        other_species: 0,
        missing_category: 0,
        isolates: 0,
        resistant: 0,
        susceptible: 0,
        groups: 0,
    };
    let mut isolates = Vec::new();
    let mut seen = BTreeSet::new();
    for record in reader.records() {
        let record = record.map_err(failed)?;
        summary.rows += 1;
        let line = record.position().map_or(0, |position| position.line());
        if record[species_column].trim() != config.species {
            summary.other_species += 1;
            continue;
        }
        let category = record[antibiotic_column].trim();
        if category.is_empty() {
            summary.missing_category += 1;
            continue;
        }
        let resistant = if config.resistant.iter().any(|value| value == category) {
            true
        } else if config.susceptible.iter().any(|value| value == category) {
            false
        } else {
            return Err(MarklabError::Validation(format!(
                "AST label table line {line}: `{}` value `{category}` is listed as neither resistant nor susceptible",
                config.antibiotic_column
            )));
        };
        let id = record[isolate_column].trim();
        validate_isolate_id(id, line)?;
        if !seen.insert(id.to_owned()) {
            return Err(MarklabError::Validation(format!(
                "AST label table line {line}: isolate `{id}` appears more than once"
            )));
        }
        let group = match group_column {
            Some(index) if record[index].trim().is_empty() => {
                return Err(MarklabError::Validation(format!(
                    "AST label table line {line}: isolate `{id}` has no group"
                )));
            }
            Some(index) => Some(record[index].trim().to_owned()),
            None => None,
        };
        if resistant {
            summary.resistant += 1;
        } else {
            summary.susceptible += 1;
        }
        isolates.push(Isolate {
            id: id.to_owned(),
            group,
            resistant,
        });
    }
    summary.isolates = isolates.len();
    Ok((isolates, summary))
}

/// Isolate IDs name spectrum files, so they are restricted to characters that cannot leave the
/// spectra directory.
fn validate_isolate_id(id: &str, line: u64) -> Result<()> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if id.is_empty()
        || id.len() > MAXIMUM_ISOLATE_ID_BYTES
        || id.starts_with('.')
        || !id.chars().all(allowed)
    {
        return Err(MarklabError::Validation(format!(
            "AST label table line {line}: isolate ID `{id}` must be 1 to \
             {MAXIMUM_ISOLATE_ID_BYTES} letters, digits, '-', '_' or '.', not starting with '.', \
             because it names the spectrum file"
        )));
    }
    Ok(())
}

/// Read each isolate's binned intensities into a row-major matrix.
fn read_features(directory: &Path, isolates: &[Isolate]) -> Result<(Layout, Vec<f64>)> {
    let mut layout: Option<Layout> = None;
    let mut features = Vec::new();
    for isolate in isolates {
        let path = directory.join(format!("{}.json", isolate.id));
        let binned = read_binned(&path)?;
        match &layout {
            None => {
                let values = isolates.len().saturating_mul(binned.layout.bins);
                if values > MAXIMUM_LOGISTIC_VALUES {
                    return Err(MarklabError::Validation(format!(
                        "{} isolates × {} bins exceed {MAXIMUM_LOGISTIC_VALUES} feature values",
                        isolates.len(),
                        binned.layout.bins
                    )));
                }
                features.reserve_exact(values);
                layout = Some(binned.layout);
            }
            Some(first) if *first != binned.layout => {
                return Err(MarklabError::Validation(format!(
                    "{}: bins differ from the first isolate's",
                    path.display()
                )));
            }
            Some(_) => {}
        }
        features.extend_from_slice(&binned.intensity);
    }
    let layout = layout.ok_or_else(|| {
        MarklabError::Validation("no isolate of the species has a listed category".to_owned())
    })?;
    Ok((layout, features))
}

fn read_binned(path: &Path) -> Result<Binned> {
    let io_error = |source| MarklabError::io(path, source);
    let unusable = || {
        MarklabError::Validation(format!(
            "{}: a spectrum must be a regular file of at most 512 MiB",
            path.display()
        ))
    };
    // Check before opening: opening a FIFO would block.
    if !fs::metadata(path).map_err(io_error)?.is_file() {
        return Err(unusable());
    }
    let file = File::open(path).map_err(io_error)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_SPECTRUM_FILE_BYTES {
        return Err(unusable());
    }
    let reader = BufReader::new(file.take(MAXIMUM_SPECTRUM_FILE_BYTES));
    let result: PreprocessResult = serde_json::from_reader(reader)
        .map_err(|error| MarklabError::Validation(format!("{}: {error}", path.display())))?;
    let binned = result.binned.ok_or_else(|| {
        MarklabError::Validation(format!(
            "{}: no binned intensities; preprocess with `binning`",
            path.display()
        ))
    })?;
    if binned.intensity.len() != binned.layout.bins {
        return Err(MarklabError::Validation(format!(
            "{}: {} intensities for {} bins",
            path.display(),
            binned.intensity.len(),
            binned.layout.bins
        )));
    }
    Ok(binned)
}
