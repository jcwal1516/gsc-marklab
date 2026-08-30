use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker, TopologyCliError};
use super::schema::MultiresolutionFactorSpec;
use super::worker_assets;

pub(super) fn run_multiresolution_factor(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: MultiresolutionFactorSpec = serde_json::from_slice(&bytes)?;
    spec.scales
        .sort_by(|left, right| left.scale_id.cmp(&right.scale_id));
    validate_multiresolution_factor(&spec)?;
    let assets = worker_assets::load("marklab_jax_multiresolution_factor_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.jax_multiresolution_factor_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_l_bfgs_laplace",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "scales": spec.scales,
        "loading_precision": spec.loading_precision,
        "noise_standard_deviation": spec.noise_standard_deviation,
        "maximum_iterations": spec.maximum_iterations,
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
    if result["format"] != "marklab.multiresolution_spatial_factor_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_multiresolution_spatial_factors"
    {
        return Err(TopologyCliError::Backend(
            "multiresolution factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_multiresolution_factor(
    spec: &MultiresolutionFactorSpec,
) -> Result<(), TopologyCliError> {
    let feature_count = spec.feature_names.len();
    let mut features = HashSet::new();
    let valid_features = spec.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && features.insert(name.as_str())
    });
    let mut entities = HashSet::new();
    let mut previous_entity: Option<&str> = None;
    let valid_rows = spec.rows.iter().all(|row| {
        let canonical_order =
            previous_entity.is_none_or(|previous| previous < row.entity_id.as_str());
        previous_entity = Some(row.entity_id.as_str());
        canonical_order
            && !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && row.values.len() == feature_count
            && row.observed.len() == feature_count
            && row.values.iter().all(|value| value.is_finite())
            && row.observed.iter().any(|value| *value)
    });
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    let mut scale_ids = HashSet::new();
    let mut physical_scales = HashSet::new();
    let valid_scales = spec.scales.iter().all(|scale| {
        !scale.scale_id.trim().is_empty()
            && scale.scale_id.trim() == scale.scale_id
            && scale_ids.insert(scale.scale_id.as_str())
            && scale.physical_scale_um.is_finite()
            && scale.physical_scale_um > 0.0
            && physical_scales.insert(scale.physical_scale_um.to_bits())
            && (1..=16).contains(&scale.basis_columns.len())
            && scale.basis_columns.iter().all(|column| {
                column.len() == spec.rows.len()
                    && column.iter().all(|value| value.is_finite())
                    && column.iter().any(|value| *value != 0.0)
            })
            && (1..=scale.basis_columns.len().min(feature_count)).contains(&scale.factors)
            && scale.coefficient_precision.is_finite()
            && scale.coefficient_precision > 0.0
    });
    let parameter_count = spec.scales.iter().fold(0usize, |count, scale| {
        count.saturating_add((scale.basis_columns.len() + feature_count) * scale.factors)
    });
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "region"
        || spec.likelihood != "gaussian"
        || !(2..=32).contains(&feature_count)
        || !valid_features
        || !(8..=256).contains(&spec.rows.len())
        || !valid_rows
        || !enough_observed
        || !spec
            .rows
            .iter()
            .any(|row| row.observed.iter().any(|value| !*value))
        || !(2..=8).contains(&spec.scales.len())
        || !valid_scales
        || parameter_count > 512
        || !spec.loading_precision.is_finite()
        || spec.loading_precision <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "multiresolution factors require a bounded region Gaussian matrix, unique declared physical scales with finite bases, masked evaluation entries, and positive Laplace controls"
                .into(),
        ));
    }
    Ok(())
}
