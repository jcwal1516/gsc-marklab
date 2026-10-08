use super::{publish_json, BayesCliError};
use marklab_bayes::{
    build_joint_location_mark_model, sha256_hex, JointCategoricalMarkSpec, JointCategoricalPoint,
    JointLocationGridRow, JointMarkError, RectangularWindow,
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
    reference: String,
    lps: f64,
    mcps: f64,
    mfps: f64,
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
    let points = read::<JointCategoricalPoint, 8>(
        &pb,
        [
            "point_id",
            "x_um",
            "y_um",
            "mark_id",
            "location_covariate",
            "location_offset",
            "mark_covariate",
            "neighborhood_effect",
        ],
    )?;
    let grid = read::<JointLocationGridRow, 4>(
        &gb,
        ["ix", "iy", "location_covariate", "location_offset"],
    )?;
    let model = build_joint_location_mark_model(JointCategoricalMarkSpec {
        points,
        grid,
        window: RectangularWindow {
            xmin_um: xmin,
            ymin_um: ymin,
            xmax_um: xmax,
            ymax_um: ymax,
        },
        grid_x: gx,
        grid_y: gy,
        reference_mark: reference,
        location_prior_sd: lps,
        mark_coefficient_prior_sd: mcps,
        mark_field_prior_sd: mfps,
    })
    .map_err(|JointMarkError::Invalid(m)| BayesCliError::Input(m))?;
    publish_json(
        &out,
        &Output {
            format: "marklab.joint_location_categorical_mark_model",
            version: 1,
            points_sha256: sha256_hex(&pb),
            grid_sha256: sha256_hex(&gb),
            fit_state: "not_fitted",
            model: model.model,
            point_count: model.point_count,
            mark_counts: model.mark_counts,
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
            "joint mark CSV headers differ from exact contract".into(),
        ));
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
    model: marklab_bayes::JointCategoricalMarkModelIr,
    point_count: u32,
    mark_counts: std::collections::BTreeMap<String, u32>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_continuous(
    pp: PathBuf,
    gp: PathBuf,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    gx: u32,
    gy: u32,
    noise: f64,
    amplitude: f64,
    length: f64,
    jitter: f64,
    loading_sd: f64,
    private_sd: f64,
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
    let points = read::<marklab_bayes::JointContinuousPoint, 7>(
        &pb,
        [
            "point_id",
            "x_um",
            "y_um",
            "mark_y",
            "location_covariate",
            "location_offset",
            "mark_covariate",
        ],
    )?;
    let grid = read::<JointLocationGridRow, 4>(
        &gb,
        ["ix", "iy", "location_covariate", "location_offset"],
    )?;
    let model =
        marklab_bayes::build_joint_continuous_mark_model(marklab_bayes::JointContinuousMarkSpec {
            points,
            grid,
            window: RectangularWindow {
                xmin_um: xmin,
                ymin_um: ymin,
                xmax_um: xmax,
                ymax_um: ymax,
            },
            grid_x: gx,
            grid_y: gy,
            known_mark_noise_sd: noise,
            shared_field_amplitude: amplitude,
            shared_field_length_scale_um: length,
            jitter,
            mark_loading_prior_sd: loading_sd,
            private_field_prior_sd: private_sd,
        })
        .map_err(|JointMarkError::Invalid(m)| BayesCliError::Input(m))?;
    publish_json(
        &out,
        &ContinuousOutput {
            format: "marklab.joint_location_continuous_mark_model",
            version: 1,
            points_sha256: sha256_hex(&pb),
            grid_sha256: sha256_hex(&gb),
            fit_state: "not_fitted",
            model: model.model,
            point_count: model.point_count,
        },
    )
}
#[derive(Serialize)]
struct ContinuousOutput {
    format: &'static str,
    version: u32,
    points_sha256: String,
    grid_sha256: String,
    fit_state: &'static str,
    model: marklab_bayes::JointContinuousMarkModelIr,
    point_count: u32,
}
