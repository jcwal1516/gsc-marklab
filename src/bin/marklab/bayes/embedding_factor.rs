use super::{publish_json, BayesCliError};
use marklab_bayes::{
    joint_location_embedding_latent_factor_model, sha256_hex, EmbeddingFactorError,
    EmbeddingFactorPoint, EmbeddingFactorSpec, JointLocationGridRow, RectangularWindow,
};
use serde::Serialize;
use std::{fs, path::PathBuf};
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    pp: PathBuf,
    gp: PathBuf,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    gx: u32,
    gy: u32,
    factors: u32,
    amplitude: f64,
    length: f64,
    jitter: f64,
    loading_sd: f64,
    noise_sd: f64,
    out: PathBuf,
) -> Result<(), BayesCliError> {
    let pb = fs::read(&pp).map_err(|source| BayesCliError::Io {
        path: pp.clone(),
        source,
    })?;
    let gb = fs::read(&gp).map_err(|source| BayesCliError::Io {
        path: gp.clone(),
        source,
    })?;
    let (points, names) = read_points(&pb)?;
    let grid = read_grid(&gb)?;
    let m = joint_location_embedding_latent_factor_model(EmbeddingFactorSpec {
        points,
        feature_names: names,
        grid,
        window: RectangularWindow {
            xmin_um: xmin,
            ymin_um: ymin,
            xmax_um: xmax,
            ymax_um: ymax,
        },
        grid_x: gx,
        grid_y: gy,
        factors,
        field_amplitude: amplitude,
        field_length_scale_um: length,
        jitter,
        loading_prior_sd: loading_sd,
        noise_prior_sd: noise_sd,
    })
    .map_err(|EmbeddingFactorError::Invalid(m)| BayesCliError::Input(m))?;
    publish_json(
        &out,
        &Output {
            format: "marklab.joint_location_embedding_factor_model",
            version: 1,
            points_sha256: sha256_hex(&pb),
            grid_sha256: sha256_hex(&gb),
            fit_state: "not_fitted",
            model: m.model,
            point_count: m.point_count,
            embedding_dimension: m.embedding_dimension,
            factor_count: m.factor_count,
            feature_names: m.feature_names,
        },
    )
}
fn read_points(b: &[u8]) -> Result<(Vec<EmbeddingFactorPoint>, Vec<String>), BayesCliError> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(b);
    let h = r.headers()?.clone();
    if h.len() < 7
        || h.iter().take(5).collect::<Vec<_>>()
            != [
                "point_id",
                "x_um",
                "y_um",
                "location_covariate",
                "location_offset",
            ]
    {
        return Err(BayesCliError::Input(
            "embedding headers require fixed prefix and embedding_* columns".into(),
        ));
    }
    let names = h.iter().skip(5).map(str::to_owned).collect::<Vec<_>>();
    if names.iter().any(|n| !n.starts_with("embedding_")) {
        return Err(BayesCliError::Input(
            "embedding columns must use embedding_* names".into(),
        ));
    }
    let mut points = Vec::new();
    for row in r.records() {
        let row = row?;
        let parse = |i: usize| {
            row[i]
                .parse::<f64>()
                .map_err(|_| BayesCliError::Input("embedding numeric value is invalid".into()))
        };
        points.push(EmbeddingFactorPoint {
            point_id: row[0].to_owned(),
            x_um: parse(1)?,
            y_um: parse(2)?,
            location_covariate: parse(3)?,
            location_offset: parse(4)?,
            embedding: (5..row.len()).map(parse).collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((points, names))
}
fn read_grid(b: &[u8]) -> Result<Vec<JointLocationGridRow>, BayesCliError> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(b);
    if r.headers()?.iter().collect::<Vec<_>>()
        != ["ix", "iy", "location_covariate", "location_offset"]
    {
        return Err(BayesCliError::Input("embedding grid headers differ".into()));
    }
    Ok(r.deserialize().collect::<Result<Vec<_>, _>>()?)
}
#[derive(Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    points_sha256: String,
    grid_sha256: String,
    fit_state: &'static str,
    model: marklab_bayes::EmbeddingFactorModelIr,
    point_count: u32,
    embedding_dimension: u32,
    factor_count: u32,
    feature_names: Vec<String>,
}
