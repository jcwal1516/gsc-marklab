use std::{fs, path::PathBuf};

use marklab_bayes::{
    sha256_hex, strauss_papangelou, strauss_statistics, StraussError, StraussPapangelou,
    StraussPoint, StraussProposal,
};
use serde::Serialize;

use super::{publish_json, BayesCliError};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    interaction_radius_um: f64,
    proposal_x_um: f64,
    proposal_y_um: f64,
    beta_per_um2: f64,
    gamma: f64,
    maximum_pair_visits: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if bytes.len() > 16 * 1_048_576 {
        return Err(BayesCliError::Input(
            "Strauss point input exceeds 16 MiB".into(),
        ));
    }
    let points = read_points(&bytes)?;
    let statistics = strauss_statistics(&points, interaction_radius_um, maximum_pair_visits)
        .map_err(map_error)?;
    let proposal = strauss_papangelou(
        &points,
        StraussProposal {
            x_um: proposal_x_um,
            y_um: proposal_y_um,
        },
        beta_per_um2,
        gamma,
        interaction_radius_um,
    )
    .map_err(map_error)?;
    publish_json(
        &output_path,
        &StraussOutput {
            format: "marklab.strauss_statistics",
            version: 1,
            coordinate_unit: "micrometer",
            input_path,
            input_sha256: sha256_hex(&bytes),
            interaction_radius_um,
            pair_boundary: "euclidean_distance_less_than_or_equal_radius",
            point_count: statistics.point_count,
            unordered_pair_visits: statistics.unordered_pair_visits,
            interacting_pair_count: statistics.interacting_pair_count,
            proposal,
            maximum_pair_visits,
            claim_status: "experimental_fixed_pattern_mechanics",
        },
    )
}

fn read_points(bytes: &[u8]) -> Result<Vec<StraussPoint>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["point_id", "x_um", "y_um"] {
        return Err(BayesCliError::Input(
            "Strauss point headers must be exactly: point_id,x_um,y_um".into(),
        ));
    }
    let mut points = Vec::new();
    for row in reader.deserialize() {
        points.push(row?);
        if points.len() > 100_000 {
            return Err(BayesCliError::Input(
                "Strauss point input exceeds 100000 rows".into(),
            ));
        }
    }
    Ok(points)
}

fn map_error(error: StraussError) -> BayesCliError {
    match error {
        StraussError::InvalidSpec(message) | StraussError::Resource(message) => {
            BayesCliError::Input(message)
        }
        StraussError::Numerical(message) => BayesCliError::Backend(message),
    }
}

#[derive(Serialize)]
struct StraussOutput {
    format: &'static str,
    version: u32,
    coordinate_unit: &'static str,
    input_path: PathBuf,
    input_sha256: String,
    interaction_radius_um: f64,
    pair_boundary: &'static str,
    point_count: u64,
    unordered_pair_visits: u64,
    interacting_pair_count: u64,
    proposal: StraussPapangelou,
    maximum_pair_visits: u64,
    claim_status: &'static str,
}
