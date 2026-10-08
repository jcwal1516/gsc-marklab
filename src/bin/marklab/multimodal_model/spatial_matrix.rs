use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker, TopologyCliError};
use super::schema::SpatialMatrixFactorSpec;
use super::worker_assets;

pub(super) fn run_spatial_matrix_factor(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: SpatialMatrixFactorSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    spec.graph
        .edges
        .sort_by(|left, right| (&left.left, &left.right).cmp(&(&right.left, &right.right)));
    validate_spatial_matrix_factor(&spec)?;
    let assets = worker_assets::load("marklab_scipy_spatial_matrix_factor_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.scipy_spatial_matrix_factor_request",
        "version": 1,
        "backend": {
            "name": "scipy_l_bfgs_laplace",
            "version": "scipy-1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "graph": spec.graph,
        "factors": spec.factors,
        "spatial_precision": spec.spatial_precision,
        "diagonal_epsilon": spec.diagonal_epsilon,
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
    if result["format"] != "marklab.spatial_bayesian_matrix_factorization"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_spatial_matrix_factorization"
    {
        return Err(TopologyCliError::Backend(
            "spatial matrix factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_spatial_matrix_factor(spec: &SpatialMatrixFactorSpec) -> Result<(), TopologyCliError> {
    let feature_count = spec.feature_names.len();
    let mut features = HashSet::new();
    let valid_features = spec.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && features.insert(name.as_str())
    });
    let mut entities = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && row.values.len() == feature_count
            && row.observed.len() == feature_count
            && row.values.iter().all(|value| value.is_finite())
            && row.observed.iter().any(|value| *value)
    });
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    let mut edge_keys = HashSet::new();
    let mut incident = HashSet::new();
    let valid_edges = spec.graph.edges.iter().all(|edge| {
        let valid = edge.left < edge.right
            && entities.contains(edge.left.as_str())
            && entities.contains(edge.right.as_str())
            && edge.weight.is_finite()
            && edge.weight > 0.0
            && edge_keys.insert((edge.left.as_str(), edge.right.as_str()));
        if valid {
            incident.insert(edge.left.as_str());
            incident.insert(edge.right.as_str());
        }
        valid
    });
    let total_parameters = (spec.rows.len() + feature_count).saturating_mul(spec.factors);
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "region"
        || spec.likelihood != "gaussian"
        || !(2..=64).contains(&feature_count)
        || !valid_features
        || !(8..=256).contains(&spec.rows.len())
        || !valid_rows
        || !spec
            .rows
            .iter()
            .any(|row| row.observed.iter().any(|value| !*value))
        || !enough_observed
        || spec.graph.graph_id.trim().is_empty()
        || spec.graph.normalization != "unnormalized_laplacian"
        || spec.graph.edges.is_empty()
        || !valid_edges
        || incident.len() != spec.rows.len()
        || !(1..=feature_count.min(spec.rows.len() - 1)).contains(&spec.factors)
        || total_parameters > 512
        || !spec.spatial_precision.is_finite()
        || spec.spatial_precision <= 0.0
        || !spec.diagonal_epsilon.is_finite()
        || spec.diagonal_epsilon <= 0.0
        || !spec.loading_precision.is_finite()
        || spec.loading_precision <= 0.0
        || !spec.noise_standard_deviation.is_finite()
        || spec.noise_standard_deviation <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "spatial matrix factorization requires a bounded region Gaussian matrix, canonical positive graph edges covering every entity, masked evaluation values, and positive Laplace controls"
                .into(),
        ));
    }
    Ok(())
}
