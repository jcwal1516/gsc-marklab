use super::cli_schema::MultitypePapangelouArgs;
use super::{publish_json, CliError};
use marklab_bayes::{
    multitype_papangelou, sha256_hex, MultitypeBaseline, MultitypeError, MultitypeInteraction,
    MultitypePapangelouResult, MultitypePapangelouSpec, MultitypePoint,
};
use serde::Serialize;
use std::fs;
pub(super) fn run(args: MultitypePapangelouArgs) -> Result<(), CliError> {
    let MultitypePapangelouArgs {
        points,
        baselines,
        interactions,
        proposal_type,
        proposal_x_um,
        proposal_y_um,
        maximum_visits,
        out,
    } = args;
    let pp = points;
    let bp = baselines;
    let ip = interactions;
    let x = proposal_x_um;
    let y = proposal_y_um;
    let cap = maximum_visits;
    let pb = fs::read(&pp).map_err(|source| CliError::Io {
        path: pp.clone(),
        source,
    })?;
    let bb = fs::read(&bp).map_err(|source| CliError::Io {
        path: bp.clone(),
        source,
    })?;
    let ib = fs::read(&ip).map_err(|source| CliError::Io {
        path: ip.clone(),
        source,
    })?;
    let points = read::<MultitypePoint, 4>(&pb, ["point_id", "x_um", "y_um", "type_id"])?;
    let baselines = read::<MultitypeBaseline, 2>(&bb, ["type_id", "log_baseline_per_um2"])?;
    let interactions = read::<MultitypeInteraction, 4>(
        &ib,
        ["type_a", "type_b", "log_pair_potential", "radius_um"],
    )?;
    let result = multitype_papangelou(MultitypePapangelouSpec {
        points,
        baselines,
        interactions,
        proposal_type,
        proposal_x_um: x,
        proposal_y_um: y,
        maximum_visits: cap,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.multitype_papangelou",
            version: 1,
            points_sha256: sha256_hex(&pb),
            baselines_sha256: sha256_hex(&bb),
            interactions_sha256: sha256_hex(&ib),
            matrix_policy: "complete_exact_symmetric",
            result,
        },
    )
}
fn read<T: for<'a> serde::Deserialize<'a>, const N: usize>(
    b: &[u8],
    h: [&str; N],
) -> Result<Vec<T>, CliError> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(b);
    if r.headers()?.iter().collect::<Vec<_>>() != h {
        return Err(CliError::Input(
            "multitype CSV headers differ from exact contract".into(),
        ));
    }
    Ok(r.deserialize().collect::<Result<Vec<_>, _>>()?)
}
fn map_error(e: MultitypeError) -> CliError {
    match e {
        MultitypeError::Invalid(m) | MultitypeError::Resource(m) => CliError::Input(m),
        MultitypeError::Numerical(m) => CliError::Backend(m),
    }
}
#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    points_sha256: String,
    baselines_sha256: String,
    interactions_sha256: String,
    matrix_policy: &'static str,
    #[serde(flatten)]
    result: MultitypePapangelouResult,
}
