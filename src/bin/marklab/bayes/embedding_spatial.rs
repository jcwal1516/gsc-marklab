use super::{publish_json, BayesCliError};
use marklab_bayes::{
    embedding_cross_covariance_by_distance, sha256_hex, vector_semivariogram,
    EmbeddingCrossCovarianceResult, EmbeddingCrossCovarianceSpec, EmbeddingDistanceBin,
    EmbeddingPairWeight, EmbeddingSpatialError, EmbeddingSpatialPoint,
    ProjectedEmbeddingInputIdentity, ProjectedEmbeddingPoint, ProjectedEmbeddingVariogramSpec,
    ProjectedEmbeddingVariogramWorkerRequest, ProjectedEmbeddingVariogramWorkerResult,
    VectorSemivariogramResult, VectorSemivariogramSpec,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use super::{run_worker, MAXIMUM_INPUT_BYTES};

pub(super) fn run_vector_semivariogram(
    input: PathBuf,
    bins: PathBuf,
    weights: Option<PathBuf>,
    maximum_pair_visits: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = read(&input)?;
    let bin_bytes = read(&bins)?;
    let weight_bytes = weights.as_ref().map(read).transpose()?;
    let (points, feature_names) = read_points(&input_bytes)?;
    let bins = read_bins(&bin_bytes)?;
    let pair_weights = weight_bytes.as_deref().map(read_weights).transpose()?;
    let result = vector_semivariogram(VectorSemivariogramSpec {
        points,
        feature_names,
        bins,
        weights: pair_weights,
        maximum_pair_visits,
    })
    .map_err(map_embedding_error)?;
    publish_json(
        &out,
        &VectorSemivariogramOutput {
            format: "marklab.vector_semivariogram",
            version: 1,
            input_sha256: sha256_hex(&input_bytes),
            bins_sha256: sha256_hex(&bin_bytes),
            weights_sha256: weight_bytes.as_deref().map(sha256_hex),
            result,
        },
    )
}

pub(super) fn run_cross_covariance(
    input: PathBuf,
    bins: PathBuf,
    maximum_pair_visits: u64,
    maximum_matrix_elements: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = read(&input)?;
    let bin_bytes = read(&bins)?;
    let (points, feature_names) = read_points(&input_bytes)?;
    let bins_value = read_bins(&bin_bytes)?;
    let result = embedding_cross_covariance_by_distance(EmbeddingCrossCovarianceSpec {
        points,
        feature_names,
        bins: bins_value,
        maximum_pair_visits,
        maximum_matrix_elements,
    })
    .map_err(map_embedding_error)?;
    publish_json(
        &out,
        &CrossCovarianceOutput {
            format: "marklab.embedding_cross_covariance_by_distance",
            version: 1,
            input_sha256: sha256_hex(&input_bytes),
            bins_sha256: sha256_hex(&bin_bytes),
            result,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_projected_variograms(
    input: PathBuf,
    bins: PathBuf,
    components: u32,
    permutations: u32,
    seed: u64,
    maximum_pair_visits: u64,
    timeout_seconds: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let input_bytes = read(&input)?;
    let bin_bytes = read(&bins)?;
    let (points, feature_names) = read_projected_points(&input_bytes)?;
    let bins_value = read_bins(&bin_bytes)?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker_directory = repository.join("workers/python");
    let lock_path = worker_directory.join("uv.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker_path = worker_directory.join("marklab_scipy_projected_variograms_worker.py");
    let worker_bytes = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = ProjectedEmbeddingVariogramWorkerRequest::new(
        ProjectedEmbeddingVariogramSpec {
            points,
            feature_names,
            bins: bins_value,
            components,
            permutations,
            seed,
            maximum_pair_visits,
            timeout_seconds,
        },
        sha256_hex(&lock_bytes),
        sha256_hex(&worker_bytes),
    )?;
    let identity = ProjectedEmbeddingInputIdentity {
        input_path: input.display().to_string(),
        input_sha256: sha256_hex(&input_bytes),
        bins_path: bins.display().to_string(),
        bins_sha256: sha256_hex(&bin_bytes),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(
        repository,
        "marklab_scipy_projected_variograms_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: ProjectedEmbeddingVariogramWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&request, &request_sha256)?;
    publish_json(&out, &result.into_result(request, identity))
}

pub(super) fn read(path: &PathBuf) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "embedding spatial inputs must be regular files within the 16 MiB limit".into(),
        ));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.clone(),
        source,
    })
}

fn read_projected_points(
    bytes: &[u8],
) -> Result<(Vec<ProjectedEmbeddingPoint>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 8
        || headers.iter().take(6).collect::<Vec<_>>()
            != [
                "object_id",
                "biological_unit",
                "split",
                "permutation_stratum",
                "x_um",
                "y_um",
            ]
    {
        return Err(BayesCliError::Input(
            "projected embedding input requires identity, split, stratum, coordinates, and at least two embedding_* columns"
                .into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(6)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut points = Vec::new();
    for record in reader.records() {
        let record = record?;
        let parse = |index: usize| {
            record[index].parse::<f64>().map_err(|_| {
                BayesCliError::Input(format!(
                    "projected embedding numeric value in column {} is invalid",
                    &headers[index]
                ))
            })
        };
        points.push(ProjectedEmbeddingPoint {
            object_id: record[0].to_owned(),
            biological_unit: record[1].to_owned(),
            split: record[2].to_owned(),
            permutation_stratum: record[3].to_owned(),
            x_um: parse(4)?,
            y_um: parse(5)?,
            embedding: (6..record.len())
                .map(parse)
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((points, feature_names))
}

pub(crate) fn read_points(
    bytes: &[u8],
) -> Result<(Vec<EmbeddingSpatialPoint>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 5
        || headers.iter().take(3).collect::<Vec<_>>() != ["object_id", "x_um", "y_um"]
    {
        return Err(BayesCliError::Input(
            "embedding input requires object_id,x_um,y_um and at least two embedding_* columns"
                .into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(3)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut points = Vec::new();
    for record in reader.records() {
        let record = record?;
        let parse = |index: usize| {
            record[index].parse::<f64>().map_err(|_| {
                BayesCliError::Input(format!(
                    "embedding numeric value in column {} is invalid",
                    &headers[index]
                ))
            })
        };
        points.push(EmbeddingSpatialPoint {
            object_id: record[0].to_owned(),
            x_um: parse(1)?,
            y_um: parse(2)?,
            embedding: (3..record.len())
                .map(parse)
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((points, feature_names))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BinRow {
    bin_id: String,
    lower_um: f64,
    upper_um: f64,
}

pub(crate) fn read_bins(bytes: &[u8]) -> Result<Vec<EmbeddingDistanceBin>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["bin_id", "lower_um", "upper_um"] {
        return Err(BayesCliError::Input(
            "distance bin headers must be bin_id,lower_um,upper_um".into(),
        ));
    }
    reader
        .deserialize::<BinRow>()
        .map(|row| {
            let row = row?;
            Ok(EmbeddingDistanceBin {
                bin_id: row.bin_id,
                lower_um: row.lower_um,
                upper_um: row.upper_um,
                upper_inclusive: false,
            })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeightRow {
    left_object_id: String,
    right_object_id: String,
    weight: f64,
}

pub(crate) fn read_weights(bytes: &[u8]) -> Result<Vec<EmbeddingPairWeight>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>()
        != ["left_object_id", "right_object_id", "weight"]
    {
        return Err(BayesCliError::Input(
            "pair weight headers must be left_object_id,right_object_id,weight".into(),
        ));
    }
    reader
        .deserialize::<WeightRow>()
        .map(|row| {
            let row = row?;
            Ok(EmbeddingPairWeight {
                left_object_id: row.left_object_id,
                right_object_id: row.right_object_id,
                weight: row.weight,
            })
        })
        .collect()
}

fn map_embedding_error(error: EmbeddingSpatialError) -> BayesCliError {
    match error {
        EmbeddingSpatialError::Invalid(message) => BayesCliError::Input(message),
        EmbeddingSpatialError::Numeric(message) => BayesCliError::Backend(message),
    }
}

#[derive(Serialize)]
struct VectorSemivariogramOutput {
    format: &'static str,
    version: u32,
    input_sha256: String,
    bins_sha256: String,
    weights_sha256: Option<String>,
    #[serde(flatten)]
    result: VectorSemivariogramResult,
}

#[derive(Serialize)]
struct CrossCovarianceOutput {
    format: &'static str,
    version: u32,
    input_sha256: String,
    bins_sha256: String,
    #[serde(flatten)]
    result: EmbeddingCrossCovarianceResult,
}
