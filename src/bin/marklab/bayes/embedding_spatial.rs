use super::{publish_json, CliError};
use marklab_bayes::{json_sha256, sha256_hex};
use marklab_embeddings::analysis::{
    embedding_cross_covariance_by_distance, vector_semivariogram, EmbeddingCrossCovarianceResult,
    EmbeddingCrossCovarianceSpec, EmbeddingDistanceBin, EmbeddingPairWeight, EmbeddingSpatialError,
    EmbeddingSpatialPoint, ProjectedEmbeddingInputIdentity, ProjectedEmbeddingPoint,
    ProjectedEmbeddingVariogramSpec, ProjectedEmbeddingVariogramWorkerRequest,
    ProjectedEmbeddingVariogramWorkerResult, VectorSemivariogramResult, VectorSemivariogramSpec,
};
use marklab_embeddings::{EmbeddingNpyMatrix, SourceBundleBudgets};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::cli_schema::EmbeddingCrossCovarianceByDistanceArgs;
use super::cli_schema::ProjectedEmbeddingVariogramsArgs;
use super::cli_schema::VectorSemivariogramArgs;
use super::worker_identity;
use super::{run_worker, MAXIMUM_INPUT_BYTES};

pub(super) fn run_vector_semivariogram(args: VectorSemivariogramArgs) -> Result<(), CliError> {
    let VectorSemivariogramArgs {
        input,
        vectors,
        bins,
        weights,
        maximum_pair_visits,
        out,
    } = args;
    let input_bytes = read(&input)?;
    let bin_bytes = read(&bins)?;
    let weight_bytes = weights.as_deref().map(read).transpose()?;
    let (points, feature_names, vectors_sha256) = if let Some(path) = vectors {
        let vectors_bytes = read(&path)?;
        let (points, feature_names, digest) = read_points_with_npy(&input_bytes, &vectors_bytes)?;
        (points, feature_names, Some(digest))
    } else {
        let (points, feature_names) = read_points(&input_bytes)?;
        (points, feature_names, None)
    };
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
            version: if vectors_sha256.is_some() { 2 } else { 1 },
            input_sha256: sha256_hex(&input_bytes),
            vectors_sha256,
            bins_sha256: sha256_hex(&bin_bytes),
            weights_sha256: weight_bytes.as_deref().map(sha256_hex),
            result,
        },
    )
}

pub(super) fn run_cross_covariance(
    args: EmbeddingCrossCovarianceByDistanceArgs,
) -> Result<(), CliError> {
    let EmbeddingCrossCovarianceByDistanceArgs {
        input,
        bins,
        maximum_pair_visits,
        maximum_matrix_elements,
        out,
    } = args;
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

pub(super) fn run_projected_variograms(
    args: ProjectedEmbeddingVariogramsArgs,
) -> Result<(), CliError> {
    let ProjectedEmbeddingVariogramsArgs {
        input,
        bins,
        components,
        permutations,
        seed,
        maximum_pair_visits,
        timeout_seconds,
        out,
    } = args;
    let prepared = prepare_projected_variograms(
        input,
        bins,
        components,
        permutations,
        seed,
        maximum_pair_visits,
        timeout_seconds,
    )?;
    let result = execute_projected_variograms(&prepared)?;
    publish_json(&out, &result)
}

pub(crate) struct PreparedProjectedVariograms {
    pub request: ProjectedEmbeddingVariogramWorkerRequest,
    pub input_path: PathBuf,
    pub input_sha256: String,
    pub bins_path: PathBuf,
    pub bins_sha256: String,
    pub request_sha256: String,
    pub timeout_seconds: u64,
}

pub(crate) fn prepare_projected_variograms(
    input: PathBuf,
    bins: PathBuf,
    components: u32,
    permutations: u32,
    seed: u64,
    maximum_pair_visits: u64,
    timeout_seconds: u64,
) -> Result<PreparedProjectedVariograms, CliError> {
    let input_bytes = read(&input)?;
    let bin_bytes = read(&bins)?;
    let (points, feature_names) = read_projected_points(&input_bytes)?;
    let bins_value = read_bins(&bin_bytes)?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let identity = worker_identity(repository, "marklab_scipy_projected_variograms_worker.py")?;
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
        identity.environment_lock_sha256,
        identity.worker_sha256,
    )?;
    let request_sha256 = json_sha256(&request)?;
    Ok(PreparedProjectedVariograms {
        request,
        input_path: input,
        input_sha256: sha256_hex(&input_bytes),
        bins_path: bins,
        bins_sha256: sha256_hex(&bin_bytes),
        request_sha256,
        timeout_seconds,
    })
}

pub(crate) fn execute_projected_variograms(
    prepared: &PreparedProjectedVariograms,
) -> Result<marklab_embeddings::analysis::ProjectedEmbeddingVariogramResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let request_bytes = serde_json::to_vec(&prepared.request)?;
    let result_bytes = run_worker(
        repository,
        "marklab_scipy_projected_variograms_worker.py",
        &request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: ProjectedEmbeddingVariogramWorkerResult = serde_json::from_slice(&result_bytes)?;
    result.validate(&prepared.request, &prepared.request_sha256)?;
    Ok(result.into_result(
        prepared.request.clone(),
        ProjectedEmbeddingInputIdentity {
            input_path: prepared.input_path.display().to_string(),
            input_sha256: prepared.input_sha256.clone(),
            bins_path: prepared.bins_path.display().to_string(),
            bins_sha256: prepared.bins_sha256.clone(),
        },
    ))
}

pub(super) fn read(path: &Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "embedding spatial inputs must be regular files within the 16 MiB limit",
    )
}

pub(crate) fn read_projected_points(
    bytes: &[u8],
) -> Result<(Vec<ProjectedEmbeddingPoint>, Vec<String>), CliError> {
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
        return Err(CliError::Input(
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
                CliError::Input(format!(
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
) -> Result<(Vec<EmbeddingSpatialPoint>, Vec<String>), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 5
        || headers.iter().take(3).collect::<Vec<_>>() != ["object_id", "x_um", "y_um"]
    {
        return Err(CliError::Input(
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
                CliError::Input(format!(
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

fn read_points_with_npy(
    input: &[u8],
    vectors: &[u8],
) -> Result<(Vec<EmbeddingSpatialPoint>, Vec<String>, String), CliError> {
    let maximum_retained = MAXIMUM_INPUT_BYTES as usize * 4;
    let matrix = EmbeddingNpyMatrix::from_bytes(
        vectors,
        SourceBundleBudgets::new(
            MAXIMUM_INPUT_BYTES,
            0,
            0,
            maximum_retained,
            MAXIMUM_INPUT_BYTES,
        ),
    )
    .map_err(|error| CliError::Input(error.to_string()))?;
    let rows = usize::try_from(matrix.row_count())
        .map_err(|_| CliError::Input("NPY row count exceeds this platform's capacity".into()))?;
    let dimension = matrix.dimension() as usize;
    let required = dimension
        .checked_mul(size_of::<f64>())
        .and_then(|bytes| bytes.checked_add(size_of::<EmbeddingSpatialPoint>()))
        .and_then(|bytes| bytes.checked_mul(rows))
        .and_then(|bytes| {
            dimension
                .checked_mul(64)
                .and_then(|names| bytes.checked_add(names))
        })
        .and_then(|bytes| {
            input
                .len()
                .checked_mul(3)
                .and_then(|text| bytes.checked_add(text))
        })
        .and_then(|bytes| bytes.checked_add(vectors.len()));
    if required.is_none_or(|required| required > maximum_retained) {
        return Err(CliError::Input(
            "NPY spatial input exceeds the 64 MiB retained-data limit".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(input);
    if !reader.headers()?.iter().eq(["object_id", "x_um", "y_um"]) {
        return Err(CliError::Input(
            "NPY spatial rows require exactly object_id,x_um,y_um columns".into(),
        ));
    }
    let mut points = Vec::new();
    points
        .try_reserve_exact(rows)
        .map_err(|_| CliError::Input("unable to allocate NPY spatial rows".into()))?;
    for (row, record) in reader.records().enumerate() {
        let record = record?;
        if row >= rows {
            return Err(CliError::Input(
                "coordinate CSV and NPY row count differ".into(),
            ));
        }
        let coordinate = |index: usize| {
            record[index].parse::<f64>().map_err(|_| {
                CliError::Input("NPY spatial coordinates must be numeric micrometres".into())
            })
        };
        let embedding = (0..matrix.dimension())
            .map(|column| {
                matrix
                    .value(row as u64, column)
                    .map(f64::from)
                    .map_err(|error| CliError::Input(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        points.push(EmbeddingSpatialPoint {
            object_id: record[0].to_owned(),
            x_um: coordinate(1)?,
            y_um: coordinate(2)?,
            embedding,
        });
    }
    if points.len() != rows {
        return Err(CliError::Input(
            "coordinate CSV and NPY row count differ".into(),
        ));
    }
    let feature_names = (0..dimension)
        .map(|column| format!("embedding_{column}"))
        .collect();
    Ok((
        points,
        feature_names,
        matrix.summary().content_digest().to_string(),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BinRow {
    bin_id: String,
    lower_um: f64,
    upper_um: f64,
}

pub(crate) fn read_bins(bytes: &[u8]) -> Result<Vec<EmbeddingDistanceBin>, CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["bin_id", "lower_um", "upper_um"] {
        return Err(CliError::Input(
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

pub(crate) fn read_weights(bytes: &[u8]) -> Result<Vec<EmbeddingPairWeight>, CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>()
        != ["left_object_id", "right_object_id", "weight"]
    {
        return Err(CliError::Input(
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

fn map_embedding_error(error: EmbeddingSpatialError) -> CliError {
    match error {
        EmbeddingSpatialError::Invalid(message) => CliError::Input(message),
        EmbeddingSpatialError::Numeric(message) => CliError::Backend(message),
    }
}

#[derive(Serialize)]
struct VectorSemivariogramOutput {
    format: &'static str,
    version: u32,
    input_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    vectors_sha256: Option<String>,
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
