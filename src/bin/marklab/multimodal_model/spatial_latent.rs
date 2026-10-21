//! One-factor spatial latent model with masked-entry prediction (PyMC).
//!
//! Features are standardized with the mean and population SD of their observed entries.
//! Observed entries follow `y_rk ~ Normal(f_r w_k, noise_standard_deviation)` on that scale, with
//! `f = L z`, `z ~ Normal(0, 1)`, and `L = chol(Matern32(d; length_scale_um) + 1e-6 I)` of unit
//! marginal variance; `length_scale_um ~ HalfNormal(length_scale_prior_um)` and
//! `w_k ~ Normal(0, 1)`. With `latent_field.kind = "spde_alpha_two"` (and `matern_nu = 1`) the
//! factor is instead `f = A w` with `w ~ Normal(0, Q^-1)`, the Lindgren-Rue-Lindstrom SPDE
//! precision of a unit-variance Matern field with smoothness 1 on a regular mesh over the
//! regions' bounding box widened by `mesh_extension_um`, `A` the barycentric projection and
//! `range_um ~ HalfNormal(length_scale_prior_um)` the practical range `sqrt(8) / kappa`.
//! The sign of `(f, w)` is aligned after sampling (largest loading
//! positive). Masked entries are excluded from fitting and predicted on the original scale.
//! `fit_state` is complete when there are no divergences and maximum rank R-hat is at most 1.1;
//! R-hat is computed before sign alignment, so chains on opposite signs read as nonconverged.

use marklab_bayes::is_worker_timeout;
use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker};
use super::schema::{SpatialLatentFactorSpec, SpatialLatentField};
use super::worker_assets;
use crate::cli_error::CliError;
use marklab_bayes::backend_versions::NUMPY_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;

pub(super) fn run_spatial_latent_factor(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let mut spec: SpatialLatentFactorSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_spatial_latent_factor(&spec)?;
    let assets = worker_assets::load("marklab_pymc_spatial_latent_factor_worker.py")?;
    let mut request = serde_json::json!({
        "format": "marklab.pymc_spatial_latent_factor_request",
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": "pymc-6.3.0",
            "pytensor_version": "3.2.4",
            "numpy_version": NUMPY_VERSION,
            "python_version": PYTHON_VERSION,
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "coordinate_frame": spec.coordinate_frame,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "factors": spec.factors,
        "matern_nu": spec.matern_nu,
        "length_scale_prior_um": spec.length_scale_prior_um,
        "noise_standard_deviation": spec.noise_standard_deviation,
        "warmup": spec.warmup,
        "samples": spec.samples,
        "target_accept": spec.target_accept,
        "seed": spec.seed
    });
    // Only an SPDE request carries the field, so dense-GP request bytes are unchanged.
    if let Some(field) = &spec.latent_field {
        request["latent_field"] = serde_json::to_value(field)?;
    }
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &assets.repository,
        &assets.worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.spatial_latent_factor_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || !matches!(
            result["fit_state"].as_str(),
            Some("complete" | "nonconverged")
        )
    {
        return Err(CliError::Backend(
            "spatial latent factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_spatial_latent_factor(spec: &SpatialLatentFactorSpec) -> Result<(), CliError> {
    let feature_count = spec.feature_names.len();
    let mut features = HashSet::new();
    let valid_features = spec.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && features.insert(name.as_str())
    });
    let mut entities = HashSet::new();
    let mut coordinates = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && row.coordinates_um.iter().all(|value| value.is_finite())
            && coordinates.insert((
                row.coordinates_um[0].to_bits(),
                row.coordinates_um[1].to_bits(),
            ))
            && row.values.len() == feature_count
            && row.observed.len() == feature_count
            && row.values.iter().all(|value| value.is_finite())
            && row.observed.iter().any(|value| *value)
    });
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "region"
        || spec.coordinate_frame.trim().is_empty()
        || spec.coordinate_frame.trim() != spec.coordinate_frame
        || spec.likelihood != "gaussian"
        || !(2..=16).contains(&feature_count)
        || !valid_features
        || !(8..=64).contains(&spec.rows.len())
        || !valid_rows
        || !enough_observed
        || !spec
            .rows
            .iter()
            .any(|row| row.observed.iter().any(|value| !*value))
        || spec.factors != 1
        || !valid_latent_field(spec)
        || !spec.length_scale_prior_um.is_finite()
        || spec.length_scale_prior_um <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(20..=2_000).contains(&spec.warmup)
        || !(20..=2_000).contains(&spec.samples)
        || !spec.target_accept.is_finite()
        || !(0.8..1.0).contains(&spec.target_accept)
        || !is_worker_timeout(spec.timeout_seconds)
    {
        return Err(CliError::Input(
            "spatial latent factors require unique finite region coordinates, a bounded Gaussian matrix with masked evaluation entries, one Matern-3/2 factor (or an SPDE factor with matern_nu 1, mesh resolution 4-24 and a finite nonnegative extension), and bounded NUTS controls"
                .into(),
        ));
    }
    Ok(())
}

fn valid_latent_field(spec: &SpatialLatentFactorSpec) -> bool {
    match &spec.latent_field {
        None => spec.matern_nu == 1.5,
        Some(SpatialLatentField::SpdeAlphaTwo {
            mesh_resolution,
            mesh_extension_um,
        }) => {
            spec.matern_nu == 1.0
                && (4..=24).contains(mesh_resolution)
                && mesh_extension_um.is_finite()
                && *mesh_extension_um >= 0.0
        }
    }
}
