use clap::ValueEnum;
use marklab_bayes::{
    car_density, validate_spatial_weights, CarDensityError, CarMode, CarSpec, DiagonalPolicy,
    IslandPolicy, NormalizationPolicy, RegionFieldValue, SpatialEdge, SpatialWeightsPolicy,
    SymmetryPolicy,
};
use serde::{Deserialize, Serialize};

use super::cli_schema::CarDensityArgs;
use super::{publish_json, CliError, MAXIMUM_INPUT_BYTES};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliCarMode {
    Proper,
    Intrinsic,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliIslandPolicy {
    Reject,
    Exclude,
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
struct FieldRow {
    region_id: String,
    value: f64,
}

pub(super) fn run(args: CarDensityArgs) -> Result<(), CliError> {
    let CarDensityArgs {
        regions,
        edges,
        field,
        mode,
        tau,
        rho,
        constraint_tolerance,
        island_policy,
        out,
    } = args;
    let region_path = regions;
    let edge_path = edges;
    let field_path = field;
    let output_path = out;
    let weights = validate_spatial_weights(
        read_regions(&region_path)?,
        read_edges(&edge_path)?,
        SpatialWeightsPolicy {
            symmetry: SymmetryPolicy::Required,
            diagonal: DiagonalPolicy::Zero,
            normalization: NormalizationPolicy::Preserve,
        },
    )
    .map_err(|error| CliError::Input(error.to_string()))?;
    let weights_digest = weights.digest_sha256.clone();
    let region_ids = weights.region_ids.clone();
    let result = car_density(CarSpec {
        mode: match mode {
            CliCarMode::Proper => CarMode::Proper,
            CliCarMode::Intrinsic => CarMode::Intrinsic,
        },
        tau,
        rho,
        constraint_tolerance,
        island_policy: match island_policy {
            CliIslandPolicy::Reject => IslandPolicy::Reject,
            CliIslandPolicy::Exclude => IslandPolicy::Exclude,
        },
        weights,
        field: read_field(&field_path)?,
    })
    .map_err(map_error)?;
    let constraints = result
        .constraints
        .iter()
        .map(|component| {
            format!(
                "sum_to_zero:{}",
                component
                    .iter()
                    .map(|&index| region_ids[index].as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>();
    let output = CarOutput {
        format: "marklab.car_density",
        version: 1,
        mode: match mode {
            CliCarMode::Proper => "proper",
            CliCarMode::Intrinsic => "intrinsic",
        },
        tau,
        rho,
        constraint_tolerance,
        island_policy: match island_policy {
            CliIslandPolicy::Reject => "reject",
            CliIslandPolicy::Exclude => "exclude",
        },
        weights_digest_sha256: weights_digest,
        log_density: result.log_density,
        rank_deficiency: result.rank_deficiency,
        constraints,
        excluded_islands: result
            .excluded_islands
            .iter()
            .map(|&index| region_ids[index].clone())
            .collect(),
        claim_status: "experimental_field_density_diagnostic",
    };
    publish_json(&output_path, &output)
}

fn read_regions(path: &std::path::Path) -> Result<Vec<String>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["region_id"]) {
        return Err(CliError::Input(
            "CAR region headers must be exactly: region_id".into(),
        ));
    }
    reader
        .deserialize::<RegionRow>()
        .map(|row| row.map(|row| row.region_id).map_err(CliError::from))
        .collect()
}

fn read_edges(path: &std::path::Path) -> Result<Vec<SpatialEdge>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["source_region", "target_region", "weight"])
    {
        return Err(CliError::Input(
            "CAR edge headers must be exactly: source_region,target_region,weight".into(),
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
            .map_err(CliError::from)
        })
        .collect()
}

fn read_field(path: &std::path::Path) -> Result<Vec<RegionFieldValue>, CliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["region_id", "value"]) {
        return Err(CliError::Input(
            "CAR field headers must be exactly: region_id,value".into(),
        ));
    }
    reader
        .deserialize::<FieldRow>()
        .map(|row| {
            row.map(|row| RegionFieldValue {
                region_id: row.region_id,
                value: row.value,
            })
            .map_err(CliError::from)
        })
        .collect()
}

fn validate_file(path: &std::path::Path) -> Result<(), CliError> {
    crate::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "CAR input must be a regular file within the 16 MiB limit",
    )
}

fn map_error(error: CarDensityError) -> CliError {
    match error {
        CarDensityError::InvalidInput(message) => CliError::Input(message),
        CarDensityError::Numerical(message) => CliError::Backend(message),
    }
}

#[derive(Debug, Serialize)]
struct CarOutput {
    format: &'static str,
    version: u32,
    mode: &'static str,
    tau: f64,
    rho: f64,
    constraint_tolerance: f64,
    island_policy: &'static str,
    weights_digest_sha256: String,
    log_density: f64,
    rank_deficiency: usize,
    constraints: Vec<String>,
    excluded_islands: Vec<String>,
    claim_status: &'static str,
}
