use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker, TopologyCliError};
use super::schema::{HierarchicalFactorSpec, MatrixFactorSpec, MofaSpec};
use super::worker_assets;

pub(super) fn run_hierarchical_factor(
    input: PathBuf,
    out: PathBuf,
) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: HierarchicalFactorSpec = serde_json::from_slice(&bytes)?;
    validate_hierarchical_factor(&spec)?;
    spec.entities
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    spec.modalities
        .sort_by(|left, right| left.modality_id.cmp(&right.modality_id));
    let latent_nodes = spec
        .entities
        .iter()
        .map(|entity| {
            serde_json::json!({
                "entity_id": entity.entity_id,
                "level": entity.level,
                "dimensions": spec.latent_dimensions,
                "prior": if entity.level == "patient" { "standard_normal" } else { "conditional_gaussian" }
            })
        })
        .collect::<Vec<_>>();
    let conditional_edges = spec
        .entities
        .iter()
        .filter_map(|entity| {
            entity.parent_id.as_ref().map(|parent| {
                serde_json::json!({
                    "parent_id": parent,
                    "child_id": entity.entity_id,
                    "child_level": entity.level,
                    "transition": "linear_gaussian"
                })
            })
        })
        .collect::<Vec<_>>();
    let observation_attachments = spec
        .modalities
        .iter()
        .flat_map(|modality| {
            modality.entity_ids.iter().map(|entity_id| {
                serde_json::json!({
                    "modality_id": modality.modality_id,
                    "entity_id": entity_id,
                    "entity_level": modality.entity_level,
                    "measurement_status": modality.measurement_status,
                    "likelihood": modality.likelihood
                })
            })
        })
        .collect::<Vec<_>>();
    let result = serde_json::json!({
        "format": "marklab.hierarchical_factor_graph",
        "version": 1,
        "model_id": spec.model_id,
        "latent_dimensions": spec.latent_dimensions,
        "replication_unit": "patient",
        "latent_nodes": latent_nodes,
        "conditional_edges": conditional_edges,
        "observation_attachments": observation_attachments,
        "direct_cell_patient_replication": false,
        "claim_status": "compiled_hierarchical_factor_graph"
    });
    publish_json(&out, &result)
}

fn validate_hierarchical_factor(spec: &HierarchicalFactorSpec) -> Result<(), TopologyCliError> {
    let mut entity_ids = HashSet::new();
    let entity_map = spec
        .entities
        .iter()
        .map(|entity| (entity.entity_id.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let valid_entities = spec.entities.iter().all(|entity| {
        let expected_parent = match entity.level.as_str() {
            "patient" => None,
            "specimen" => Some("patient"),
            "region" => Some("specimen"),
            "cell" => Some("region"),
            _ => return false,
        };
        let parent_level = entity
            .parent_id
            .as_deref()
            .and_then(|parent| entity_map.get(parent))
            .map(|parent| parent.level.as_str());
        !entity.entity_id.trim().is_empty()
            && entity.entity_id.trim() == entity.entity_id
            && entity_ids.insert(entity.entity_id.as_str())
            && parent_level == expected_parent
    });
    let mut modality_ids = HashSet::new();
    let valid_modalities = spec.modalities.iter().all(|modality| {
        let mut attachments = HashSet::new();
        !modality.modality_id.trim().is_empty()
            && modality.modality_id.trim() == modality.modality_id
            && modality_ids.insert(modality.modality_id.as_str())
            && matches!(
                modality.entity_level.as_str(),
                "patient" | "specimen" | "region" | "cell"
            )
            && matches!(
                modality.measurement_status.as_str(),
                "measured" | "imported_prediction" | "morphology_prediction"
            )
            && matches!(
                modality.likelihood.as_str(),
                "gaussian"
                    | "bernoulli"
                    | "binomial"
                    | "poisson"
                    | "negative_binomial"
                    | "ordinal"
                    | "categorical"
            )
            && !modality.entity_ids.is_empty()
            && modality.entity_ids.iter().all(|entity_id| {
                attachments.insert(entity_id.as_str())
                    && entity_map
                        .get(entity_id.as_str())
                        .is_some_and(|entity| entity.level == modality.entity_level)
            })
    });
    if spec.model_id.trim().is_empty()
        || spec.model_id.trim() != spec.model_id
        || !(1..=32).contains(&spec.latent_dimensions)
        || spec.entities.is_empty()
        || spec.entities.len() > 100_000
        || !spec.entities.iter().any(|entity| entity.level == "patient")
        || !valid_entities
        || spec.modalities.is_empty()
        || !valid_modalities
    {
        return Err(TopologyCliError::Input(
            "hierarchical factors require exact patient-specimen-region-cell parentage and level-matched measured or predicted modality attachments"
                .into(),
        ));
    }
    Ok(())
}

pub(super) fn run_matrix_factor(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: MatrixFactorSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_matrix_factor(&spec)?;
    let assets = worker_assets::load("marklab_mofapy2_matrix_factor_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.mofapy2_matrix_factor_request",
        "version": 1,
        "backend": {
            "name": "mofapy2",
            "version": "mofapy2-0.7.4",
            "h5py_version": "3.16.0",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "LGPL-3.0",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "matrix_id": spec.matrix_id,
        "entity_level": spec.entity_level,
        "likelihood": spec.likelihood,
        "feature_names": spec.feature_names,
        "rows": spec.rows,
        "factors": spec.factors,
        "iterations": spec.iterations,
        "convergence_mode": spec.convergence_mode,
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
    if result["format"] != "marklab.bayesian_matrix_factorization"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_bayesian_matrix_factorization"
    {
        return Err(TopologyCliError::Backend(
            "matrix factor result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_matrix_factor(spec: &MatrixFactorSpec) -> Result<(), TopologyCliError> {
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
    let has_masked = spec
        .rows
        .iter()
        .any(|row| row.observed.iter().any(|value| !*value));
    let enough_observed = (0..feature_count)
        .all(|feature| spec.rows.iter().filter(|row| row.observed[feature]).count() >= 8);
    if spec.matrix_id.trim().is_empty()
        || spec.matrix_id.trim() != spec.matrix_id
        || spec.entity_level != "patient"
        || spec.likelihood != "gaussian"
        || !(2..=128).contains(&feature_count)
        || !valid_features
        || !(8..=10_000).contains(&spec.rows.len())
        || !valid_rows
        || !has_masked
        || !enough_observed
        || !(1..=feature_count.min(spec.rows.len() - 1)).contains(&spec.factors)
        || !(50..=10_000).contains(&spec.iterations)
        || !matches!(spec.convergence_mode.as_str(), "fast" | "medium" | "slow")
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "matrix factorization requires a bounded patient Gaussian matrix, exact identifiers, observed training values, masked evaluation values, and bounded fit controls"
                .into(),
        ));
    }
    Ok(())
}

pub(super) fn run_mofa(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: MofaSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    validate_mofa(&spec)?;
    let assets = worker_assets::load("marklab_mofapy2_multiview_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.mofapy2_multiview_request",
        "version": 1,
        "backend": {
            "name": "mofapy2",
            "version": "mofapy2-0.7.4",
            "h5py_version": "3.16.0",
            "numpy_version": "2.4.6",
            "scipy_version": "1.18.1",
            "python_version": "3.12",
            "license": "LGPL-3.0",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "design": spec.design,
        "rows": spec.rows,
        "maximum_factors": spec.maximum_factors,
        "iterations": spec.iterations,
        "convergence_mode": spec.convergence_mode,
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
    if result["format"] != "marklab.multiview_factor_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["design"]["validation_status"] != "passed"
        || result["claim_status"] != "experimental_synthetic_multiview_factor_model"
    {
        return Err(TopologyCliError::Backend(
            "MOFA result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_mofa(spec: &MofaSpec) -> Result<(), TopologyCliError> {
    let train_count = spec.rows.iter().filter(|row| row.split == "train").count();
    let test_count = spec.rows.iter().filter(|row| row.split == "test").count();
    let total_features = spec
        .design
        .modalities
        .iter()
        .map(|modality| modality.feature_names.len())
        .sum::<usize>();
    let mut modality_ids = HashSet::new();
    let valid_modalities = spec.design.modalities.iter().all(|modality| {
        let mut features = HashSet::new();
        !modality.id.trim().is_empty()
            && modality.id.trim() == modality.id
            && modality_ids.insert(modality.id.as_str())
            && modality.measurement_status == "measured"
            && modality.likelihood == "gaussian"
            && (1..=128).contains(&modality.feature_names.len())
            && modality.feature_names.iter().all(|feature| {
                !feature.trim().is_empty()
                    && feature.trim() == feature
                    && features.insert(feature.as_str())
            })
    });
    let mut entity_ids = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entity_ids.insert(row.entity_id.as_str())
            && matches!(row.split.as_str(), "train" | "test")
            && row.views.len() == spec.design.modalities.len()
            && row
                .views
                .iter()
                .zip(&spec.design.modalities)
                .all(|(view, modality)| {
                    view.values.len() == modality.feature_names.len()
                        && view.observed.len() == modality.feature_names.len()
                        && view.values.iter().all(|value| value.is_finite())
                })
            && (row.split != "test"
                || (row
                    .views
                    .iter()
                    .any(|view| view.observed.iter().any(|value| *value))
                    && row
                        .views
                        .iter()
                        .any(|view| view.observed.iter().any(|value| !*value))))
    });
    if spec.design.entity_level != "patient"
        || spec.design.missingness_assumption != "structurally_absent_or_mar"
        || spec
            .design
            .coordinate_frame
            .as_ref()
            .is_some_and(|frame| frame.trim().is_empty())
        || !(2..=8).contains(&spec.design.modalities.len())
        || !valid_modalities
        || !valid_rows
        || train_count < 12
        || test_count == 0
        || !(1..=total_features.min(train_count - 1)).contains(&spec.maximum_factors)
        || !(50..=10_000).contains(&spec.iterations)
        || !matches!(spec.convergence_mode.as_str(), "fast" | "medium" | "slow")
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "MOFA requires valid measured patient Gaussian views, structural/MAR masks, train/held-out rows, and bounded fit controls"
                .into(),
        ));
    }
    Ok(())
}
