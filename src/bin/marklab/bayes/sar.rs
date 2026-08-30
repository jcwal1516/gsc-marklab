use std::{collections::BTreeMap, path::PathBuf};

use clap::ValueEnum;
use marklab_bayes::{
    sar_gaussian_log_likelihood, validate_spatial_weights, DiagonalPolicy, NormalizationPolicy,
    SarError, SarModelType, SarSpec, SpatialEdge, SpatialWeightsPolicy, SymmetryPolicy,
    ValidatedSpatialWeights,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliSarModel {
    Lag,
    Error,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliSarInterpretation {
    Descriptive,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegionRow {
    region_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeRow {
    source_region: String,
    target_region: String,
    weight: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoefficientRow {
    predictor: String,
    coefficient: f64,
}

pub(super) struct SarInput {
    pub weights: ValidatedSpatialWeights,
    pub predictor_names: Vec<String>,
    pub response: Vec<f64>,
    pub design: Vec<f64>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    region_path: PathBuf,
    edge_path: PathBuf,
    data_path: PathBuf,
    coefficient_path: PathBuf,
    model: CliSarModel,
    rho: f64,
    sigma: f64,
    interpretation: CliSarInterpretation,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input = read_input(&region_path, &edge_path, &data_path)?;
    let weights = input.weights;
    let region_ids = weights.region_ids.clone();
    let weights_digest_sha256 = weights.digest_sha256.clone();
    let predictor_names = input.predictor_names;
    let response = input.response;
    let design = input.design;
    let (intercept, coefficients) = read_coefficients(&coefficient_path, &predictor_names)?;
    let result = sar_gaussian_log_likelihood(SarSpec {
        model_type: match model {
            CliSarModel::Lag => SarModelType::Lag,
            CliSarModel::Error => SarModelType::Error,
        },
        response,
        design,
        predictor_names: predictor_names.clone(),
        intercept,
        coefficients: coefficients.clone(),
        rho,
        sigma,
        weights,
    })
    .map_err(map_error)?;
    let coefficients = predictor_names
        .into_iter()
        .zip(coefficients)
        .map(|(predictor, coefficient)| CoefficientOutput {
            predictor,
            coefficient,
        })
        .collect();
    let impacts = result
        .impacts
        .into_iter()
        .map(|impact| ImpactOutput {
            predictor: impact.predictor,
            direct: impact.direct,
            indirect: impact.indirect,
            total: impact.total,
        })
        .collect();
    publish_json(
        &output_path,
        &SarOutput {
            format: "marklab.sar_likelihood",
            version: 1,
            regions_path: region_path,
            edges_path: edge_path,
            data_path,
            coefficients_path: coefficient_path,
            model_type: match model {
                CliSarModel::Lag => "lag",
                CliSarModel::Error => "error",
            },
            interpretation: match interpretation {
                CliSarInterpretation::Descriptive => "descriptive",
            },
            region_count: region_ids.len(),
            weights_digest_sha256,
            rho,
            sigma,
            intercept,
            coefficients,
            log_abs_determinant: result.log_abs_determinant,
            residual_sum_squares: result.residual_sum_squares,
            log_likelihood: result.log_likelihood,
            impacts,
            claim_status: "experimental_fixed_parameter_likelihood",
        },
    )
}

pub(super) fn read_input(
    region_path: &std::path::Path,
    edge_path: &std::path::Path,
    data_path: &std::path::Path,
) -> Result<SarInput, BayesCliError> {
    let weights = validate_spatial_weights(
        read_regions(region_path)?,
        read_edges(edge_path)?,
        SpatialWeightsPolicy {
            symmetry: SymmetryPolicy::NotRequired,
            diagonal: DiagonalPolicy::Zero,
            normalization: NormalizationPolicy::RowStandardize,
        },
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let (predictor_names, response, design) = read_data(data_path, &weights.region_ids)?;
    Ok(SarInput {
        weights,
        predictor_names,
        response,
        design,
    })
}

fn read_regions(path: &std::path::Path) -> Result<Vec<String>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["region_id"]) {
        return Err(BayesCliError::Input(
            "SAR region headers must be exactly: region_id".into(),
        ));
    }
    reader
        .deserialize::<RegionRow>()
        .map(|row| row.map(|row| row.region_id).map_err(BayesCliError::from))
        .collect()
}

fn read_edges(path: &std::path::Path) -> Result<Vec<SpatialEdge>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["source_region", "target_region", "weight"])
    {
        return Err(BayesCliError::Input(
            "SAR edge headers must be exactly: source_region,target_region,weight".into(),
        ));
    }
    reader
        .deserialize::<EdgeRow>()
        .map(|row| {
            row.map(|row| SpatialEdge {
                source_region: row.source_region,
                target_region: row.target_region,
                weight: row.weight,
            })
            .map_err(BayesCliError::from)
        })
        .collect()
}

type SarData = (Vec<String>, Vec<f64>, Vec<f64>);

fn read_data(path: &std::path::Path, region_ids: &[String]) -> Result<SarData, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    let headers = reader.headers()?.clone();
    if headers.len() < 3
        || headers.len() > 34
        || headers.get(0) != Some("region_id")
        || headers.get(1) != Some("y")
    {
        return Err(BayesCliError::Input(
            "SAR data must have region_id,y followed by 1-32 predictors".into(),
        ));
    }
    let predictor_names = headers
        .iter()
        .skip(2)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if predictor_names.iter().enumerate().any(|(index, name)| {
        name.is_empty()
            || name.trim() != name
            || name == "intercept"
            || predictor_names[..index].contains(name)
    }) {
        return Err(BayesCliError::Input(
            "SAR predictor names must be exact, unique, and not intercept".into(),
        ));
    }
    let mut rows = BTreeMap::new();
    for record in reader.records() {
        let record = record?;
        let region_id = record[0].to_owned();
        let values = record
            .iter()
            .skip(1)
            .map(|value| {
                value.parse::<f64>().map_err(|_| {
                    BayesCliError::Input(
                        "SAR response and predictors must be finite numbers".into(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if values.iter().any(|value| !value.is_finite()) || rows.insert(region_id, values).is_some()
        {
            return Err(BayesCliError::Input(
                "SAR data require unique regions and finite values".into(),
            ));
        }
    }
    if rows.len() != region_ids.len() {
        return Err(BayesCliError::Input(
            "SAR data must contain exactly one row per region".into(),
        ));
    }
    let mut response = Vec::with_capacity(region_ids.len());
    let mut design = Vec::with_capacity(region_ids.len() * predictor_names.len());
    for region_id in region_ids {
        let values = rows
            .remove(region_id)
            .ok_or_else(|| BayesCliError::Input(format!("missing SAR region {region_id}")))?;
        response.push(values[0]);
        design.extend_from_slice(&values[1..]);
    }
    Ok((predictor_names, response, design))
}

fn read_coefficients(
    path: &std::path::Path,
    predictor_names: &[String],
) -> Result<(f64, Vec<f64>), BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["predictor", "coefficient"]) {
        return Err(BayesCliError::Input(
            "SAR coefficient headers must be exactly: predictor,coefficient".into(),
        ));
    }
    let mut values = BTreeMap::new();
    for row in reader.deserialize::<CoefficientRow>() {
        let row = row?;
        if row.predictor.is_empty()
            || row.predictor.trim() != row.predictor
            || !row.coefficient.is_finite()
            || values.insert(row.predictor, row.coefficient).is_some()
        {
            return Err(BayesCliError::Input(
                "SAR coefficients require exact unique names and finite values".into(),
            ));
        }
    }
    if values.len() != predictor_names.len() + 1 {
        return Err(BayesCliError::Input(
            "SAR coefficients must contain exactly intercept and every predictor".into(),
        ));
    }
    let intercept = values
        .remove("intercept")
        .ok_or_else(|| BayesCliError::Input("SAR intercept coefficient is missing".into()))?;
    let coefficients = predictor_names
        .iter()
        .map(|predictor| {
            values.remove(predictor).ok_or_else(|| {
                BayesCliError::Input(format!("missing SAR coefficient for {predictor}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((intercept, coefficients))
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "SAR input must be a regular file within the 16 MiB limit",
    )
}

fn map_error(error: SarError) -> BayesCliError {
    match error {
        SarError::InvalidInput(message) => BayesCliError::Input(message),
        SarError::Numerical(message) => BayesCliError::Backend(message),
    }
}

#[derive(Debug, Serialize)]
struct CoefficientOutput {
    predictor: String,
    coefficient: f64,
}

#[derive(Debug, Serialize)]
struct ImpactOutput {
    predictor: String,
    direct: f64,
    indirect: f64,
    total: f64,
}

#[derive(Debug, Serialize)]
struct SarOutput {
    format: &'static str,
    version: u32,
    regions_path: PathBuf,
    edges_path: PathBuf,
    data_path: PathBuf,
    coefficients_path: PathBuf,
    model_type: &'static str,
    interpretation: &'static str,
    region_count: usize,
    weights_digest_sha256: String,
    rho: f64,
    sigma: f64,
    intercept: f64,
    coefficients: Vec<CoefficientOutput>,
    log_abs_determinant: f64,
    residual_sum_squares: f64,
    log_likelihood: f64,
    impacts: Vec<ImpactOutput>,
    claim_status: &'static str,
}
