use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker, TopologyCliError};
use super::schema::SpatialLatentFactorSpec;
use super::worker_assets;

pub(super) fn run_spatial_latent_factor(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: SpatialLatentFactorSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_spatial_latent_factor(&spec)?;
    let assets = worker_assets::load("marklab_pymc_spatial_latent_factor_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.pymc_spatial_latent_factor_request",
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": "pymc-6.3.0",
            "pytensor_version": "3.2.4",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
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
        return Err(TopologyCliError::Backend(
            "spatial latent factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_spatial_latent_factor(spec: &SpatialLatentFactorSpec) -> Result<(), TopologyCliError> {
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
        || spec.matern_nu != 1.5
        || !spec.length_scale_prior_um.is_finite()
        || spec.length_scale_prior_um <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(20..=2_000).contains(&spec.warmup)
        || !(20..=2_000).contains(&spec.samples)
        || !spec.target_accept.is_finite()
        || !(0.8..1.0).contains(&spec.target_accept)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "spatial latent factors require unique finite region coordinates, a bounded Gaussian matrix with masked evaluation entries, one Matern-3/2 factor, and bounded NUTS controls"
                .into(),
        ));
    }
    Ok(())
}
