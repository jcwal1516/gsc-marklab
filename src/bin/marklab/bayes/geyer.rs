use super::{publish_json, BayesCliError};
use marklab_bayes::{
    geyer_saturation_statistic, sha256_hex, GeyerPointSummary, StraussError, StraussPoint,
};
use serde::Serialize;
use std::{fs, path::PathBuf};
pub(super) fn run(
    input: PathBuf,
    radius: f64,
    saturation: u32,
    cap: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let bytes = fs::read(&input).map_err(|source| BayesCliError::Io {
        path: input.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes.as_slice());
    if reader.headers()?.iter().collect::<Vec<_>>() != ["point_id", "x_um", "y_um"] {
        return Err(BayesCliError::Input(
            "Geyer headers must be exactly: point_id,x_um,y_um".into(),
        ));
    }
    let mut points = Vec::<StraussPoint>::new();
    for row in reader.deserialize() {
        points.push(row?);
    }
    let r = geyer_saturation_statistic(&points, radius, saturation, cap).map_err(map_error)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.geyer_saturation_statistic",
            version: 1,
            input_path: input,
            input_sha256: sha256_hex(&bytes),
            convention: "sum_over_points_of_min_saturation_and_inclusive_radius_neighbor_count",
            unordered_pair_visits: r.unordered_pair_visits,
            interacting_pair_count: r.interacting_pair_count,
            statistic: r.statistic,
            points: r.points,
        },
    )
}
fn map_error(e: StraussError) -> BayesCliError {
    match e {
        StraussError::InvalidSpec(m) | StraussError::Resource(m) => BayesCliError::Input(m),
        StraussError::Numerical(m) => BayesCliError::Backend(m),
    }
}
#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    input_path: PathBuf,
    input_sha256: String,
    convention: &'static str,
    unordered_pair_visits: u64,
    interacting_pair_count: u64,
    statistic: u64,
    points: Vec<GeyerPointSummary>,
}
