use super::{publish_json, BayesCliError};
use marklab_bayes::{
    posterior_predictive_point_process_diagnostics, sha256_hex, PointProcessPpcError,
    PointProcessPpcSpec, PpcPoint, PpcReplicatedPoint, RectangularWindow,
};
use serde::Serialize;
use std::{fs, path::PathBuf};
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    op: PathBuf,
    rp: PathBuf,
    radii_path: PathBuf,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    alpha: f64,
    cap: u64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let ob = fs::read(&op).map_err(|source| BayesCliError::Io {
        path: op.clone(),
        source,
    })?;
    let rb = fs::read(&rp).map_err(|source| BayesCliError::Io {
        path: rp.clone(),
        source,
    })?;
    let radb = fs::read(&radii_path).map_err(|source| BayesCliError::Io {
        path: radii_path.clone(),
        source,
    })?;
    let observed = read::<PpcPoint, 4>(&ob, ["pattern_id", "point_id", "x_um", "y_um"])?;
    let replicated = read::<PpcReplicatedPoint, 5>(
        &rb,
        ["replicate", "pattern_id", "point_id", "x_um", "y_um"],
    )?;
    let radii = read_radii(&radb)?;
    let r = posterior_predictive_point_process_diagnostics(PointProcessPpcSpec {
        observed,
        replicated,
        radii_um: radii,
        window: RectangularWindow {
            xmin_um: xmin,
            ymin_um: ymin,
            xmax_um: xmax,
            ymax_um: ymax,
        },
        alpha,
        maximum_pair_visits: cap,
    })
    .map_err(map_error)?;
    publish_json(
        &out,
        &Output {
            format: "marklab.point_process_posterior_predictive_diagnostics",
            version: 1,
            observed_sha256: sha256_hex(&ob),
            replicated_sha256: sha256_hex(&rb),
            radii_sha256: sha256_hex(&radb),
            pattern_count: r.pattern_count,
            replicate_count: r.replicate_count,
            observed_total_count: r.observed_total_count,
            replicated_total_count_mean: r.replicated_total_count_mean,
            replicated_total_count_sd: r.replicated_total_count_sd,
            pair_visits: r.pair_visits,
            alpha: r.alpha,
            curve: r.curve,
            exceeded_radii_um: r.exceeded_radii_um,
            interpretation: "posterior_predictive_consistency_is_not_model_truth",
        },
    )
}
fn read<T: for<'a> serde::Deserialize<'a>, const N: usize>(
    b: &[u8],
    h: [&str; N],
) -> Result<Vec<T>, BayesCliError> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(b);
    if r.headers()?.iter().collect::<Vec<_>>() != h {
        return Err(BayesCliError::Input(
            "point-process PPC headers differ".into(),
        ));
    }
    Ok(r.deserialize().collect::<Result<Vec<_>, _>>()?)
}
fn read_radii(b: &[u8]) -> Result<Vec<f64>, BayesCliError> {
    let mut r = csv::ReaderBuilder::new().has_headers(true).from_reader(b);
    if r.headers()?.iter().collect::<Vec<_>>() != ["radius_um"] {
        return Err(BayesCliError::Input("PPC radii header differs".into()));
    }
    r.records()
        .map(|row| {
            row.map_err(Into::into).and_then(|x| {
                x[0].parse()
                    .map_err(|_| BayesCliError::Input("invalid radius".into()))
            })
        })
        .collect()
}
fn map_error(e: PointProcessPpcError) -> BayesCliError {
    match e {
        PointProcessPpcError::Invalid(m) | PointProcessPpcError::Resource(m) => {
            BayesCliError::Input(m)
        }
        PointProcessPpcError::Numerical(m) => BayesCliError::Backend(m),
    }
}
#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    observed_sha256: String,
    replicated_sha256: String,
    radii_sha256: String,
    pattern_count: u32,
    replicate_count: u32,
    observed_total_count: u64,
    replicated_total_count_mean: f64,
    replicated_total_count_sd: f64,
    pair_visits: u64,
    alpha: f64,
    curve: Vec<marklab_bayes::PointProcessPpcCurveRow>,
    exceeded_radii_um: Vec<f64>,
    interpretation: &'static str,
}
