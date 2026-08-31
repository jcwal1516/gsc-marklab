use std::{collections::HashSet, path::PathBuf};

use marklab_topology::sha256_hex;
use serde::{Deserialize, Serialize};

use super::super::topology::{
    publish_json, read_input, read_required, run_worker, TopologyCliError,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasRepresentation {
    modality: String,
    model_version: String,
    stain: String,
    feature_names: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasAlignment {
    method: String,
    distance: String,
    ood_distance_threshold: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasReferenceSample {
    sample_id: String,
    patient_id: String,
    site_id: String,
    domain: String,
    features: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasQuery {
    query_id: String,
    features: Vec<f64>,
    expected_domain: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AtlasPerturbation {
    perturbation_id: String,
    feature_shift: Vec<f64>,
    subsample_fraction: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AtlasSpec {
    atlas_id: String,
    representation: AtlasRepresentation,
    alignment: AtlasAlignment,
    reference_samples: Vec<AtlasReferenceSample>,
    queries: Vec<AtlasQuery>,
    perturbations: Vec<AtlasPerturbation>,
    covariance_ridge: f64,
    seed: u64,
    timeout_seconds: u64,
}
pub(super) fn run_atlas(input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let mut spec: AtlasSpec = serde_json::from_slice(&bytes)?;
    spec.reference_samples
        .sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    spec.perturbations
        .sort_by(|left, right| left.perturbation_id.cmp(&right.perturbation_id));
    validate_atlas(&spec)?;
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_atlas_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = serde_json::json!({
        "format": "marklab.scipy_atlas_request",
        "version": 1,
        "backend": {
            "name": "scipy_biological_similarity_atlas",
            "scipy_version": "1.18.1",
            "numpy_version": "2.4.6",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": sha256_hex(&lock),
            "worker_sha256": sha256_hex(&worker)
        },
        "atlas_id": spec.atlas_id,
        "representation": spec.representation,
        "alignment": spec.alignment,
        "reference_samples": spec.reference_samples,
        "queries": spec.queries,
        "perturbations": spec.perturbations,
        "covariance_ridge": spec.covariance_ridge,
        "seed": spec.seed
    });
    let request_bytes = serde_json::to_vec(&request)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: serde_json::Value = serde_json::from_slice(&response)?;
    if result["format"] != "marklab.spatial_atlas_mapping_and_validation"
        || result["atlas"]["format"] != "marklab.spatial_atlas"
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&request_bytes)
        || result["claim_status"] != "experimental_synthetic_biological_similarity_atlas"
    {
        return Err(TopologyCliError::Backend(
            "atlas result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

fn validate_atlas(spec: &AtlasSpec) -> Result<(), TopologyCliError> {
    let feature_count = spec.representation.feature_names.len();
    let mut feature_names = HashSet::new();
    let valid_feature_names = spec.representation.feature_names.iter().all(|name| {
        !name.trim().is_empty() && name.trim() == name && feature_names.insert(name.as_str())
    });
    let mut sample_ids = HashSet::new();
    let mut patients = HashSet::new();
    let mut sites = HashSet::new();
    let mut domains = HashSet::new();
    let valid_samples = spec.reference_samples.iter().all(|sample| {
        patients.insert(sample.patient_id.as_str());
        sites.insert(sample.site_id.as_str());
        domains.insert(sample.domain.as_str());
        !sample.sample_id.trim().is_empty()
            && sample.sample_id.trim() == sample.sample_id
            && sample_ids.insert(sample.sample_id.as_str())
            && !sample.patient_id.trim().is_empty()
            && sample.patient_id.trim() == sample.patient_id
            && !sample.site_id.trim().is_empty()
            && sample.site_id.trim() == sample.site_id
            && !sample.domain.trim().is_empty()
            && sample.domain.trim() == sample.domain
            && sample.features.len() == feature_count
            && sample.features.iter().all(|value| value.is_finite())
    });
    let replicated_domains = domains.iter().all(|domain| {
        let domain_patients = spec
            .reference_samples
            .iter()
            .filter(|sample| sample.domain == *domain)
            .map(|sample| sample.patient_id.as_str())
            .collect::<HashSet<_>>();
        domain_patients.len() >= 4
    });
    let mut query_ids = HashSet::new();
    let valid_queries = spec.queries.iter().all(|query| {
        !query.query_id.trim().is_empty()
            && query.query_id.trim() == query.query_id
            && query_ids.insert(query.query_id.as_str())
            && query.features.len() == feature_count
            && query.features.iter().all(|value| value.is_finite())
            && query
                .expected_domain
                .as_deref()
                .is_none_or(|domain| domains.contains(domain))
    });
    let mut perturbation_ids = HashSet::new();
    let valid_perturbations = spec.perturbations.iter().all(|perturbation| {
        !perturbation.perturbation_id.trim().is_empty()
            && perturbation.perturbation_id.trim() == perturbation.perturbation_id
            && perturbation_ids.insert(perturbation.perturbation_id.as_str())
            && perturbation.feature_shift.len() == feature_count
            && perturbation
                .feature_shift
                .iter()
                .all(|value| value.is_finite())
            && perturbation.subsample_fraction.is_finite()
            && (0.5..=1.0).contains(&perturbation.subsample_fraction)
    });
    if spec.atlas_id.trim().is_empty()
        || spec.atlas_id.trim() != spec.atlas_id
        || spec.representation.modality != "measured_region_features"
        || spec.representation.model_version.trim().is_empty()
        || spec.representation.stain.trim().is_empty()
        || !(1..=128).contains(&feature_count)
        || !valid_feature_names
        || spec.alignment.method != "biological_similarity_no_physical_registration"
        || spec.alignment.distance != "shrinkage_mahalanobis"
        || !spec.alignment.ood_distance_threshold.is_finite()
        || spec.alignment.ood_distance_threshold <= 0.0
        || !(8..=10_000).contains(&spec.reference_samples.len())
        || !valid_samples
        || patients.len() < 4
        || sites.is_empty()
        || !(2..=32).contains(&domains.len())
        || !replicated_domains
        || spec.queries.is_empty()
        || !valid_queries
        || spec.perturbations.is_empty()
        || !valid_perturbations
        || !spec.covariance_ridge.is_finite()
        || spec.covariance_ridge <= 0.0
        || !(1..=3_600).contains(&spec.timeout_seconds)
    {
        return Err(TopologyCliError::Input(
            "atlas mapping requires replicated measured region domains/patients, frozen representation identity, shrinkage-Mahalanobis biological alignment, bounded queries, OOD threshold, and declared perturbations"
                .into(),
        ));
    }
    Ok(())
}
