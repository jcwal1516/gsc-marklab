use std::{collections::BTreeMap, fs, path::PathBuf};

use marklab_bayes::{
    build_icar_plan, sha256_hex, validate_spatial_weights, BymFitSpec, BymFitWorkerRequest,
    BymFitWorkerResult, BymInputIdentity, DiagonalPolicy, IcarPlan, NormalizationPolicy,
    NutsSamplingSpec, SpatialEdge, SpatialWeightsPolicy, SymmetryPolicy, ValidatedSpatialWeights,
};
use serde::Deserialize;

use super::{publish_json, run_worker, BayesCliError, MAXIMUM_INPUT_BYTES};

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

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    region_path: PathBuf,
    edge_path: PathBuf,
    data_path: PathBuf,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_sd: f64,
    structured_sd_prior: f64,
    unstructured_sd_prior: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let input = read_input(&region_path, &edge_path, &data_path)?;
    let weights = input.weights;
    let plan = input.plan;
    let data = input.data;
    let repository = &marklab::python_backend_assets_root()?;
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_pymc_bym_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let data_sha256 = sha256_hex(&serde_json::to_vec(&serde_json::json!({
        "region_ids": weights.region_ids,
        "counts": data.counts,
        "expected": data.expected,
        "design": data.design,
        "predictor_names": data.predictor_names,
    }))?);
    let request = BymFitWorkerRequest::new(
        BymFitSpec {
            region_ids: weights.region_ids,
            weights_digest_sha256: weights.digest_sha256.clone(),
            icar_transform: plan.transform,
            rank_deficiency: plan.rank_deficiency,
            components: plan.components,
            counts: data.counts,
            expected: data.expected,
            design: data.design,
            predictor_names: data.predictor_names,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            structured_sd_prior,
            unstructured_sd_prior,
        },
        sampling,
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
        timeout_seconds,
    )?;
    let identity = BymInputIdentity {
        regions_path: region_path.display().to_string(),
        edges_path: edge_path.display().to_string(),
        data_path: data_path.display().to_string(),
        weights_digest_sha256: request.weights_digest_sha256.clone(),
        data_sha256,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_pymc_bym_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: BymFitWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&output_path, &result.into_fit(request, identity))
}

pub(super) struct BymInput {
    pub weights: ValidatedSpatialWeights,
    pub plan: IcarPlan,
    pub data: BymData,
}

pub(super) fn read_input(
    region_path: &std::path::Path,
    edge_path: &std::path::Path,
    data_path: &std::path::Path,
) -> Result<BymInput, BayesCliError> {
    let weights = validate_spatial_weights(
        read_regions(region_path)?,
        read_edges(edge_path)?,
        SpatialWeightsPolicy {
            symmetry: SymmetryPolicy::Required,
            diagonal: DiagonalPolicy::Zero,
            normalization: NormalizationPolicy::Preserve,
        },
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let plan =
        build_icar_plan(&weights).map_err(|error| BayesCliError::Input(error.to_string()))?;
    let data = read_data(data_path, &weights.region_ids)?;
    Ok(BymInput {
        weights,
        plan,
        data,
    })
}

fn read_regions(path: &std::path::Path) -> Result<Vec<String>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["region_id"]) {
        return Err(BayesCliError::Input(
            "BYM region headers must be exactly: region_id".into(),
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
            "BYM edge headers must be exactly: source_region,target_region,weight".into(),
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

pub(super) struct BymData {
    pub counts: Vec<u64>,
    pub expected: Vec<f64>,
    pub design: Vec<f64>,
    pub predictor_names: Vec<String>,
}

fn read_data(path: &std::path::Path, region_ids: &[String]) -> Result<BymData, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    let headers = reader.headers()?.clone();
    if headers.len() < 4
        || headers.len() > 19
        || headers.get(0) != Some("region_id")
        || headers.get(1) != Some("count")
        || headers.get(2) != Some("expected")
    {
        return Err(BayesCliError::Input(
            "BYM data must have region_id,count,expected followed by 1-16 predictors".into(),
        ));
    }
    let predictor_names = headers
        .iter()
        .skip(3)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if predictor_names.iter().enumerate().any(|(index, name)| {
        name.is_empty()
            || name.trim() != name
            || name == "intercept"
            || predictor_names[..index].contains(name)
    }) {
        return Err(BayesCliError::Input(
            "BYM predictor names must be exact, unique, and not intercept".into(),
        ));
    }
    let mut rows = BTreeMap::new();
    for record in reader.records() {
        let record = record?;
        let region_id = record[0].to_owned();
        let count = record[1]
            .parse::<u64>()
            .map_err(|_| BayesCliError::Input("BYM counts must be nonnegative integers".into()))?;
        let values = record
            .iter()
            .skip(2)
            .map(|value| {
                value.parse::<f64>().map_err(|_| {
                    BayesCliError::Input("BYM expected/design values must be finite numbers".into())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !values[0].is_finite()
            || values[0] <= 0.0
            || values.iter().any(|value| !value.is_finite())
            || rows.insert(region_id, (count, values)).is_some()
        {
            return Err(BayesCliError::Input(
                "BYM rows require unique regions, positive expected, and finite design".into(),
            ));
        }
    }
    if rows.len() != region_ids.len() {
        return Err(BayesCliError::Input(
            "BYM data must contain exactly one row per region".into(),
        ));
    }
    let mut counts = Vec::with_capacity(region_ids.len());
    let mut expected = Vec::with_capacity(region_ids.len());
    let mut design = Vec::with_capacity(region_ids.len() * predictor_names.len());
    for region_id in region_ids {
        let (count, values) = rows
            .remove(region_id)
            .ok_or_else(|| BayesCliError::Input(format!("missing BYM region {region_id}")))?;
        counts.push(count);
        expected.push(values[0]);
        design.extend_from_slice(&values[1..]);
    }
    Ok(BymData {
        counts,
        expected,
        design,
        predictor_names,
    })
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "BYM input must be a regular file within the 16 MiB limit",
    )
}
