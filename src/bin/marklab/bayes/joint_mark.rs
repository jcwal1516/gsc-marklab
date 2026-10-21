use super::cli_schema::BuildJointContinuousMarkModelArgs;
use super::cli_schema::BuildJointLocationMarkModelArgs;
use super::{publish_json, CliError};
use marklab_bayes::{
    build_joint_location_mark_model, sha256_hex, JointCategoricalMarkSpec, JointCategoricalPoint,
    JointLocationGridRow, JointMarkError, RectangularWindow,
};
use serde::Serialize;
use std::fs;
pub(super) fn run(args: BuildJointLocationMarkModelArgs) -> Result<(), CliError> {
    let BuildJointLocationMarkModelArgs {
        points,
        grid,
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        reference_mark,
        location_prior_sd,
        mark_coefficient_prior_sd,
        mark_field_prior_sd,
        out,
    } = args;
    let pp = points;
    let gp = grid;
    let xmin = xmin_um;
    let ymin = ymin_um;
    let xmax = xmax_um;
    let ymax = ymax_um;
    let gx = grid_x;
    let gy = grid_y;
    let reference = reference_mark;
    let lps = location_prior_sd;
    let mcps = mark_coefficient_prior_sd;
    let mfps = mark_field_prior_sd;
    let pb = fs::read(&pp).map_err(|source| CliError::Io {
        path: pp.clone(),
        source,
    })?;
    let gb = fs::read(&gp).map_err(|source| CliError::Io {
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
    .map_err(|JointMarkError::Invalid(m)| CliError::Input(m))?;
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
) -> Result<Vec<T>, CliError> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(b);
    if r.headers()?.iter().collect::<Vec<_>>() != h {
        return Err(CliError::Input(
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

pub(super) fn run_continuous(args: BuildJointContinuousMarkModelArgs) -> Result<(), CliError> {
    let BuildJointContinuousMarkModelArgs {
        points,
        grid,
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        known_mark_noise_sd,
        shared_field_amplitude,
        shared_field_length_scale_um,
        jitter,
        mark_loading_prior_sd,
        private_field_prior_sd,
        out,
    } = args;
    let pp = points;
    let gp = grid;
    let xmin = xmin_um;
    let ymin = ymin_um;
    let xmax = xmax_um;
    let ymax = ymax_um;
    let gx = grid_x;
    let gy = grid_y;
    let noise = known_mark_noise_sd;
    let amplitude = shared_field_amplitude;
    let length = shared_field_length_scale_um;
    let loading_sd = mark_loading_prior_sd;
    let private_sd = private_field_prior_sd;
    let pb = fs::read(&pp).map_err(|source| CliError::Io {
        path: pp.clone(),
        source,
    })?;
    let gb = fs::read(&gp).map_err(|source| CliError::Io {
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
        .map_err(|JointMarkError::Invalid(m)| CliError::Input(m))?;
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
