use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use marklab_cohort::{
    build_spatial_fingerprint, fingerprint_distance, region_compatibility,
    FingerprintDistanceResult, RegionCompatibilityResult, SpatialFingerprint,
    SpatialFingerprintComponent, SpatialFingerprintInput,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, validate_input_file, CohortError};

#[derive(Debug, Deserialize)]
struct ComponentRow {
    sample_id: String,
    component: String,
    axis: f64,
    value: f64,
    uncertainty: f64,
    weight: f64,
}

#[derive(Default)]
struct ComponentRows {
    weight: Option<f64>,
    points: Vec<(f64, f64, f64)>,
}

pub(super) fn run(
    input: PathBuf,
    left_sample: String,
    right_sample: String,
    spec_version: String,
    out: PathBuf,
) -> Result<(), CohortError> {
    let (left, right) = read_pair(&input, &left_sample, &right_sample, &spec_version)?;
    let distance = fingerprint_distance(&left, &right)?;
    publish_json(
        &out,
        &FingerprintDistanceOutput::from_result(input, left, right, distance),
    )
}

pub(super) fn run_compatibility(
    input: PathBuf,
    left_sample: String,
    right_sample: String,
    spec_version: String,
    out: PathBuf,
) -> Result<(), CohortError> {
    let (left, right) = read_pair(&input, &left_sample, &right_sample, &spec_version)?;
    let result = region_compatibility(&left, &right)?;
    publish_json(
        &out,
        &RegionCompatibilityOutput::from_result(input, left, right, result),
    )
}

fn read_pair(
    input: &Path,
    left_sample: &str,
    right_sample: &str,
    spec_version: &str,
) -> Result<(SpatialFingerprint, SpatialFingerprint), CohortError> {
    if left_sample == right_sample {
        return Err(CohortError::Input(
            "fingerprint samples must be distinct".into(),
        ));
    }
    validate_input_file(input)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(input)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    if !headers.iter().eq([
        "sample_id",
        "component",
        "axis",
        "value",
        "uncertainty",
        "weight",
    ]) {
        return Err(CohortError::Input(
            "CSV header must be exactly sample_id,component,axis,value,uncertainty,weight".into(),
        ));
    }
    let mut samples = BTreeMap::<String, BTreeMap<String, ComponentRows>>::new();
    for decoded in reader.deserialize::<ComponentRow>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        if row.sample_id != left_sample && row.sample_id != right_sample {
            return Err(CohortError::Input(format!(
                "undeclared fingerprint sample {:?}",
                row.sample_id
            )));
        }
        let component = samples
            .entry(row.sample_id)
            .or_default()
            .entry(row.component)
            .or_default();
        if let Some(weight) = component.weight {
            if weight.to_bits() != row.weight.to_bits() {
                return Err(CohortError::Input(
                    "fingerprint component rows have conflicting weights".into(),
                ));
            }
        } else {
            component.weight = Some(row.weight);
        }
        component
            .points
            .push((row.axis, row.value, row.uncertainty));
    }
    if samples.len() != 2
        || !samples.contains_key(left_sample)
        || !samples.contains_key(right_sample)
    {
        return Err(CohortError::Input(
            "fingerprint input must contain both declared samples".into(),
        ));
    }

    let left = build(&mut samples, left_sample, spec_version)?;
    let right = build(&mut samples, right_sample, spec_version)?;
    Ok((left, right))
}

fn build(
    samples: &mut BTreeMap<String, BTreeMap<String, ComponentRows>>,
    sample_id: &str,
    spec_version: &str,
) -> Result<SpatialFingerprint, CohortError> {
    let components = samples
        .remove(sample_id)
        .ok_or_else(|| CohortError::Input(format!("missing sample {sample_id:?}")))?
        .into_iter()
        .map(|(name, mut rows)| {
            rows.points
                .sort_by(|left, right| left.0.total_cmp(&right.0));
            if rows.points.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                return Err(CohortError::Input(format!(
                    "sample {sample_id:?} component {name:?} has duplicate or non-increasing axes"
                )));
            }
            let mut axis = Vec::with_capacity(rows.points.len());
            let mut values = Vec::with_capacity(rows.points.len());
            let mut uncertainties = Vec::with_capacity(rows.points.len());
            for (axis_value, value, uncertainty) in rows.points {
                axis.push(axis_value);
                values.push(value);
                uncertainties.push(uncertainty);
            }
            let weight = rows.weight.ok_or_else(|| {
                CohortError::Input(format!(
                    "sample {sample_id:?} component {name:?} has no rows"
                ))
            })?;
            Ok(SpatialFingerprintComponent {
                name,
                axis,
                values,
                uncertainties,
                weight,
            })
        })
        .collect::<Result<Vec<_>, CohortError>>()?;
    build_spatial_fingerprint(SpatialFingerprintInput {
        sample_id: sample_id.to_owned(),
        spec_version: spec_version.to_owned(),
        components,
    })
    .map_err(Into::into)
}

#[derive(Debug, Serialize)]
struct FingerprintDistanceOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    specification: SpecificationOutput,
    left_fingerprint: FingerprintOutput,
    right_fingerprint: FingerprintOutput,
    components: Vec<ComponentDistanceOutput>,
    total_distance: f64,
}

impl FingerprintDistanceOutput {
    fn from_result(
        input: PathBuf,
        left: SpatialFingerprint,
        right: SpatialFingerprint,
        distance: FingerprintDistanceResult,
    ) -> Self {
        Self {
            format: "marklab.spatial_fingerprint_distance",
            version: 1,
            input,
            specification: SpecificationOutput {
                version: left.spec_version.clone(),
                digest: distance.specification_digest,
                normalization: left.normalization,
                uncertainty_weighting: left.uncertainty_weighting,
                missing_component_policy: left.missing_component_policy,
                training_transform: left.training_transform,
            },
            left_fingerprint: FingerprintOutput::from_fingerprint(left),
            right_fingerprint: FingerprintOutput::from_fingerprint(right),
            components: distance
                .components
                .into_iter()
                .map(|component| ComponentDistanceOutput {
                    component: component.component,
                    weight: component.weight,
                    distance: component.distance,
                    weighted_contribution: component.weighted_contribution,
                })
                .collect(),
            total_distance: distance.total_distance,
        }
    }
}

#[derive(Debug, Serialize)]
struct SpecificationOutput {
    version: String,
    digest: String,
    normalization: &'static str,
    uncertainty_weighting: &'static str,
    missing_component_policy: &'static str,
    training_transform: &'static str,
}

#[derive(Debug, Serialize)]
struct FingerprintOutput {
    sample_id: String,
    digest: String,
    components: Vec<ComponentOutput>,
}

impl FingerprintOutput {
    fn from_fingerprint(fingerprint: SpatialFingerprint) -> Self {
        Self {
            sample_id: fingerprint.sample_id,
            digest: fingerprint.digest,
            components: fingerprint
                .components
                .into_iter()
                .map(|component| ComponentOutput {
                    component: component.name,
                    axis: component.axis,
                    values: component.values,
                    uncertainties: component.uncertainties,
                    weight: component.weight,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ComponentOutput {
    component: String,
    axis: Vec<f64>,
    values: Vec<f64>,
    uncertainties: Vec<f64>,
    weight: f64,
}

#[derive(Debug, Serialize)]
struct ComponentDistanceOutput {
    component: String,
    weight: f64,
    distance: f64,
    weighted_contribution: f64,
}

#[derive(Debug, Serialize)]
struct RegionCompatibilityOutput {
    format: &'static str,
    version: u32,
    input: PathBuf,
    specification: SpecificationOutput,
    left_fingerprint: FingerprintOutput,
    right_fingerprint: FingerprintOutput,
    components: Vec<CompatibilityComponentOutput>,
    total_distance: f64,
    total_distance_standard_uncertainty: Option<f64>,
    uncertainty_method: &'static str,
    inferential_scope: &'static str,
}

impl RegionCompatibilityOutput {
    fn from_result(
        input: PathBuf,
        left: SpatialFingerprint,
        right: SpatialFingerprint,
        result: RegionCompatibilityResult,
    ) -> Self {
        let components = result
            .distance
            .components
            .iter()
            .zip(&result.components)
            .map(|(distance, uncertainty)| CompatibilityComponentOutput {
                component: distance.component.clone(),
                weight: distance.weight,
                distance: distance.distance,
                weighted_contribution: distance.weighted_contribution,
                distance_standard_uncertainty: uncertainty.distance_standard_uncertainty,
            })
            .collect();
        Self {
            format: "marklab.region_compatibility",
            version: 1,
            input,
            specification: SpecificationOutput {
                version: left.spec_version.clone(),
                digest: result.distance.specification_digest.clone(),
                normalization: left.normalization,
                uncertainty_weighting: left.uncertainty_weighting,
                missing_component_policy: left.missing_component_policy,
                training_transform: left.training_transform,
            },
            left_fingerprint: FingerprintOutput::from_fingerprint(left),
            right_fingerprint: FingerprintOutput::from_fingerprint(right),
            components,
            total_distance: result.distance.total_distance,
            total_distance_standard_uncertainty: result.total_distance_standard_uncertainty,
            uncertainty_method: result.uncertainty_method,
            inferential_scope: result.inferential_scope,
        }
    }
}

#[derive(Debug, Serialize)]
struct CompatibilityComponentOutput {
    component: String,
    weight: f64,
    distance: f64,
    weighted_contribution: f64,
    distance_standard_uncertainty: Option<f64>,
}
