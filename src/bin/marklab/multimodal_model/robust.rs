use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;

use super::super::topology::{publish_json, read_input, run_worker, TopologyCliError};
use super::schema::{DropoutRobustSpec, JointPathologySpec};
use super::worker_assets;

pub(super) fn run_joint_pathology(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: JointPathologySpec = serde_json::from_slice(&bytes)?;
    spec.patients.sort();
    spec.regions
        .sort_by(|left, right| left.region_id.cmp(&right.region_id));
    spec.region_observations
        .sort_by(|left, right| left.region_id.cmp(&right.region_id));
    spec.patient_outcomes
        .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
    validate_joint_pathology(&spec)?;
    let assets = worker_assets::load("marklab_jax_joint_pathology_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.jax_joint_pathology_request",
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
        "project_id": spec.project_id,
        "patients": spec.patients,
        "regions": spec.regions,
        "region_observations": spec.region_observations,
        "patient_outcomes": spec.patient_outcomes,
        "model_spec": spec.model_spec,
        "inference_plan": spec.inference_plan,
        "region_latent_standard_deviation": spec.region_latent_standard_deviation,
        "gaussian_noise_standard_deviation": spec.gaussian_noise_standard_deviation,
        "parameter_precision": spec.parameter_precision,
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
    if result["format"] != "marklab.joint_pathology_model_fit"
        || result["model_ir"]["format"] != "marklab.joint_pathology_model_ir"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_joint_pathology_model"
    {
        return Err(TopologyCliError::Backend(
            "joint pathology result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_joint_pathology(spec: &JointPathologySpec) -> Result<(), TopologyCliError> {
    let mut patients = HashSet::new();
    let valid_patients = spec.patients.iter().all(|patient| {
        !patient.trim().is_empty() && patient.trim() == patient && patients.insert(patient.as_str())
    });
    let mut regions = HashSet::new();
    let valid_regions = spec.regions.iter().all(|region| {
        !region.region_id.trim().is_empty()
            && region.region_id.trim() == region.region_id
            && regions.insert(region.region_id.as_str())
            && patients.contains(region.patient_id.as_str())
    });
    let mut observed_regions = HashSet::new();
    let valid_region_observations = spec.region_observations.iter().all(|observation| {
        regions.contains(observation.region_id.as_str())
            && observed_regions.insert(observation.region_id.as_str())
            && observation.morphology.len() == spec.model_spec.morphology.feature_names.len()
            && observation.ihc.len() == spec.model_spec.ihc.feature_names.len()
            && observation.omics_counts.len() == spec.model_spec.omics.feature_names.len()
            && observation
                .morphology
                .iter()
                .chain(&observation.ihc)
                .all(|value| value.is_finite())
            && observation.library_size.is_finite()
            && observation.library_size > 0.0
            && observation.clone_label <= 1
    });
    let mut outcome_patients = HashSet::new();
    let valid_outcomes = spec.patient_outcomes.iter().all(|outcome| {
        patients.contains(outcome.patient_id.as_str())
            && outcome_patients.insert(outcome.patient_id.as_str())
            && outcome.value.is_finite()
    });
    let blocks = [
        (&spec.model_spec.morphology, "gaussian"),
        (&spec.model_spec.ihc, "gaussian"),
        (&spec.model_spec.omics, "poisson"),
        (&spec.model_spec.clone, "bernoulli"),
        (&spec.model_spec.clinical, "gaussian"),
    ];
    let valid_blocks = blocks.iter().all(|(block, likelihood)| {
        let mut names = HashSet::new();
        block.measurement_status == "measured"
            && block.likelihood == *likelihood
            && !block.feature_names.is_empty()
            && block.feature_names.len() <= 32
            && block.feature_names.iter().all(|name| {
                !name.trim().is_empty() && name.trim() == name && names.insert(name.as_str())
            })
    }) && spec.model_spec.clone.feature_names.len() == 1
        && spec.model_spec.clinical.feature_names.len() == 1;
    if spec.project_id.trim().is_empty()
        || spec.project_id.trim() != spec.project_id
        || !(4..=64).contains(&spec.patients.len())
        || !valid_patients
        || !(spec.patients.len()..=256).contains(&spec.regions.len())
        || !valid_regions
        || spec.region_observations.len() != spec.regions.len()
        || !valid_region_observations
        || observed_regions.len() != regions.len()
        || spec.patient_outcomes.len() != spec.patients.len()
        || !valid_outcomes
        || outcome_patients.len() != patients.len()
        || spec
            .patient_outcomes
            .iter()
            .filter(|outcome| outcome.observed)
            .count()
            < 3
        || !spec
            .patient_outcomes
            .iter()
            .any(|outcome| !outcome.observed)
        || !valid_blocks
        || spec.inference_plan != "laplace"
        || !spec.region_latent_standard_deviation.is_finite()
        || spec.region_latent_standard_deviation <= 0.0
        || !spec.gaussian_noise_standard_deviation.is_finite()
        || spec.gaussian_noise_standard_deviation <= 0.0
        || !spec.parameter_precision.is_finite()
        || spec.parameter_precision <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "joint pathology requires complete patient-region ownership, five measured typed likelihood blocks, masked patient outcomes, and bounded Laplace controls"
                .into(),
        ));
    }
    Ok(())
}

pub(super) fn run_dropout_robust(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: DropoutRobustSpec = serde_json::from_slice(&bytes)?;
    spec.rows
        .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    spec.dropout_patterns
        .sort_by(|left, right| left.pattern_id.cmp(&right.pattern_id));
    spec.required_anchor_modalities.sort();
    validate_dropout_robust(&spec)?;
    let assets = worker_assets::load("marklab_jax_modality_dropout_worker.py")?;
    let request = serde_json::json!({
        "format": "marklab.jax_modality_dropout_request",
        "version": 1,
        "backend": {
            "name": "jax_scipy_predictive_objective",
            "jax_version": "0.11.1",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "Apache-2.0_plus_BSD-3-Clause",
            "environment_lock_sha256": assets.lock_sha256,
            "worker_sha256": assets.worker_sha256
        },
        "design": spec.design,
        "rows": spec.rows,
        "latent_dimensions": spec.latent_dimensions,
        "dropout_patterns": spec.dropout_patterns,
        "required_anchor_modalities": spec.required_anchor_modalities,
        "consistency_weight": spec.consistency_weight,
        "parameter_precision": spec.parameter_precision,
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
    if result["format"] != "marklab.modality_robust_inference_model"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["fit_state"] != "approximate_only"
        || result["claim_status"] != "experimental_synthetic_modality_dropout_robustness"
    {
        return Err(TopologyCliError::Backend(
            "modality dropout result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_dropout_robust(spec: &DropoutRobustSpec) -> Result<(), TopologyCliError> {
    let train_count = spec.rows.iter().filter(|row| row.split == "train").count();
    let test_count = spec.rows.iter().filter(|row| row.split == "test").count();
    let mut modality_ids = HashSet::new();
    let valid_modalities = spec.design.modalities.iter().all(|modality| {
        let mut features = HashSet::new();
        !modality.id.trim().is_empty()
            && modality.id.trim() == modality.id
            && modality_ids.insert(modality.id.as_str())
            && modality.measurement_status == "measured"
            && modality.likelihood == "gaussian"
            && (1..=64).contains(&modality.feature_names.len())
            && modality.feature_names.iter().all(|feature| {
                !feature.trim().is_empty()
                    && feature.trim() == feature
                    && features.insert(feature.as_str())
            })
    });
    let mut entities = HashSet::new();
    let valid_rows = spec.rows.iter().all(|row| {
        !row.entity_id.trim().is_empty()
            && row.entity_id.trim() == row.entity_id
            && entities.insert(row.entity_id.as_str())
            && matches!(row.split.as_str(), "train" | "test")
            && row.views.len() == spec.design.modalities.len()
            && row
                .views
                .iter()
                .zip(&spec.design.modalities)
                .all(|(view, modality)| {
                    view.values.len() == modality.feature_names.len()
                        && view.values.iter().all(|value| value.is_finite())
                })
    });
    let anchors = spec
        .required_anchor_modalities
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let valid_anchors = !anchors.is_empty()
        && anchors.len() == spec.required_anchor_modalities.len()
        && anchors.iter().all(|anchor| modality_ids.contains(anchor));
    let mut pattern_ids = HashSet::new();
    let valid_patterns = spec.dropout_patterns.iter().all(|pattern| {
        let retained = pattern
            .retained_modalities
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        !pattern.pattern_id.trim().is_empty()
            && pattern.pattern_id.trim() == pattern.pattern_id
            && pattern_ids.insert(pattern.pattern_id.as_str())
            && !retained.is_empty()
            && retained.len() == pattern.retained_modalities.len()
            && retained.len() < spec.design.modalities.len()
            && retained.iter().all(|id| modality_ids.contains(id))
            && retained.iter().any(|id| anchors.contains(id))
            && pattern.probability.is_finite()
            && pattern.probability > 0.0
    });
    let probability_sum = spec
        .dropout_patterns
        .iter()
        .map(|pattern| pattern.probability)
        .sum::<f64>();
    let maximum_latent = spec
        .design
        .modalities
        .iter()
        .map(|modality| modality.feature_names.len())
        .min()
        .unwrap_or(0);
    if spec.design.entity_level != "patient"
        || spec.design.missingness_assumption != "structurally_absent_or_mar"
        || spec.design.coordinate_frame.is_some()
        || !(2..=8).contains(&spec.design.modalities.len())
        || !valid_modalities
        || !valid_rows
        || train_count < 12
        || test_count < 4
        || !(1..=maximum_latent).contains(&spec.latent_dimensions)
        || !(1..=32).contains(&spec.dropout_patterns.len())
        || !valid_anchors
        || !valid_patterns
        || (probability_sum - 1.0).abs() > 1e-12
        || !spec.consistency_weight.is_finite()
        || spec.consistency_weight < 0.0
        || !spec.parameter_precision.is_finite()
        || spec.parameter_precision <= 0.0
        || !(10..=10_000).contains(&spec.maximum_iterations)
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "modality dropout requires measured patient Gaussian views, isolated train/test rows, normalized anchor-preserving missing-view patterns, and bounded optimization controls"
                .into(),
        ));
    }
    Ok(())
}
