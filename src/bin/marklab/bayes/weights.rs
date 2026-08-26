use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::ValueEnum;
use marklab_bayes::{
    validate_spatial_weights, DiagonalPolicy, NormalizationPolicy, SpatialEdge,
    SpatialWeightsError, SpatialWeightsPolicy, SymmetryPolicy,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliSymmetry {
    Required,
    NotRequired,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliDiagonal {
    Zero,
    Allow,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliNormalization {
    Preserve,
    RowStandardize,
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

pub(super) fn run(
    region_path: PathBuf,
    edge_path: PathBuf,
    symmetry: CliSymmetry,
    diagonal: CliDiagonal,
    normalization: CliNormalization,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let region_ids = read_regions(&region_path)?;
    let edges = read_edges(&edge_path)?;
    let policy = SpatialWeightsPolicy {
        symmetry: match symmetry {
            CliSymmetry::Required => SymmetryPolicy::Required,
            CliSymmetry::NotRequired => SymmetryPolicy::NotRequired,
        },
        diagonal: match diagonal {
            CliDiagonal::Zero => DiagonalPolicy::Zero,
            CliDiagonal::Allow => DiagonalPolicy::Allow,
        },
        normalization: match normalization {
            CliNormalization::Preserve => NormalizationPolicy::Preserve,
            CliNormalization::RowStandardize => NormalizationPolicy::RowStandardize,
        },
    };
    let validated = validate_spatial_weights(region_ids, edges, policy).map_err(map_error)?;
    let row_sums = validated
        .region_ids
        .iter()
        .cloned()
        .zip(validated.row_sums())
        .collect::<BTreeMap<_, _>>();
    let result = WeightsOutput {
        format: "marklab.validated_spatial_weights",
        version: 1,
        region_path: region_path.display().to_string(),
        edge_path: edge_path.display().to_string(),
        regions: validated.region_ids.len(),
        edges: validated.weights.len(),
        symmetry: validated.policy.symmetry,
        diagonal: validated.policy.diagonal,
        normalization: validated.policy.normalization,
        components: validated.components.len(),
        islands: validated
            .islands
            .iter()
            .map(|&index| validated.region_ids[index].clone())
            .collect(),
        row_sums,
        digest_sha256: validated.digest_sha256,
        claim_status: "foundational_graph_contract",
    };
    publish_json(&output_path, &result)
}

fn read_regions(path: &std::path::Path) -> Result<Vec<String>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["region_id"]) {
        return Err(BayesCliError::Input(
            "region headers must be exactly: region_id".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<RegionRow>() {
        rows.push(row?.region_id);
        if rows.len() > 100_000 {
            return Err(BayesCliError::Input("region count exceeds 100000".into()));
        }
    }
    Ok(rows)
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
            "edge headers must be exactly: source_region,target_region,weight".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<EdgeRow>() {
        let row = row?;
        rows.push(SpatialEdge {
            source_region: row.source_region,
            target_region: row.target_region,
            weight: row.weight,
        });
        if rows.len() > 2_000_000 {
            return Err(BayesCliError::Input("edge count exceeds 2000000".into()));
        }
    }
    Ok(rows)
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "spatial-weights input must be a regular file within the 16 MiB limit".into(),
        ));
    }
    Ok(())
}

fn map_error(error: SpatialWeightsError) -> BayesCliError {
    match error {
        SpatialWeightsError::InvalidInput(message) => BayesCliError::Input(message),
    }
}

#[derive(Debug, Serialize)]
struct WeightsOutput {
    format: &'static str,
    version: u32,
    region_path: String,
    edge_path: String,
    regions: usize,
    edges: usize,
    symmetry: SymmetryPolicy,
    diagonal: DiagonalPolicy,
    normalization: NormalizationPolicy,
    components: usize,
    islands: Vec<String>,
    row_sums: BTreeMap<String, f64>,
    digest_sha256: String,
    claim_status: &'static str,
}
