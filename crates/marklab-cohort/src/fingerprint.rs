use std::collections::HashSet;

use sha2::{Digest, Sha256};

use super::{compensated_sum, CohortInferenceError};

const MAXIMUM_COMPONENTS: usize = 4_096;
const MAXIMUM_COMPONENT_VALUES: usize = 1_000_000;
const NORMALIZATION: &str = "none";
const UNCERTAINTY_WEIGHTING: &str = "none";
const MISSING_COMPONENT_POLICY: &str = "reject";
const TRAINING_TRANSFORM: &str = "none";

/// One explicit common-axis component of a versioned spatial fingerprint.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialFingerprintComponent {
    /// Stable component name.
    pub name: String,
    /// Strictly increasing finite physical axis.
    pub axis: Vec<f64>,
    /// Finite values aligned to `axis`.
    pub values: Vec<f64>,
    /// Finite nonnegative uncertainties aligned to `axis`.
    pub uncertainties: Vec<f64>,
    /// Finite positive prespecified distance weight.
    pub weight: f64,
}

/// Input for canonical version-one spatial fingerprint construction.
#[derive(Clone, Debug)]
pub struct SpatialFingerprintInput {
    /// Exact non-empty sample identity.
    pub sample_id: String,
    /// Exact non-empty fingerprint specification version.
    pub spec_version: String,
    /// Explicit named components; construction canonicalizes name order.
    pub components: Vec<SpatialFingerprintComponent>,
}

/// Canonical structured spatial fingerprint.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialFingerprint {
    /// Exact sample identity.
    pub sample_id: String,
    /// Specification version.
    pub spec_version: String,
    /// Fixed normalization policy.
    pub normalization: &'static str,
    /// Fixed uncertainty-weighting policy.
    pub uncertainty_weighting: &'static str,
    /// Fixed missing-component policy.
    pub missing_component_policy: &'static str,
    /// Fixed training-transform policy.
    pub training_transform: &'static str,
    /// Canonically name-ordered structured components.
    pub components: Vec<SpatialFingerprintComponent>,
    /// SHA-256 of the component specification independent of sample values.
    pub specification_digest: String,
    /// SHA-256 of specification, sample identity, values, and uncertainties.
    pub digest: String,
}

/// One component's weighted-L2 distance decomposition.
#[derive(Clone, Debug, PartialEq)]
pub struct FingerprintComponentDistance {
    /// Exact component name.
    pub component: String,
    /// Prespecified component weight.
    pub weight: f64,
    /// Unweighted common-axis L2 distance.
    pub distance: f64,
    /// `weight * distance` contribution to the total.
    pub weighted_contribution: f64,
}

/// Compatible fingerprint distance with explicit component contributions.
#[derive(Clone, Debug, PartialEq)]
pub struct FingerprintDistanceResult {
    /// Shared specification digest.
    pub specification_digest: String,
    /// Left fingerprint content digest.
    pub left_digest: String,
    /// Right fingerprint content digest.
    pub right_digest: String,
    /// Canonically ordered component contributions.
    pub components: Vec<FingerprintComponentDistance>,
    /// Stable sum of weighted component distances.
    pub total_distance: f64,
}

/// First-order uncertainty propagation for one fingerprint component distance.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionCompatibilityComponent {
    /// Exact component name.
    pub component: String,
    /// Unweighted common-axis L2 distance.
    pub distance: f64,
    /// Propagated standard uncertainty, or `None` at a zero-distance
    /// nondifferentiable point with nonzero endpoint uncertainty.
    pub distance_standard_uncertainty: Option<f64>,
}

/// Descriptive within-patient compatibility result.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionCompatibilityResult {
    /// Canonical distance and component decomposition.
    pub distance: FingerprintDistanceResult,
    /// Component-level first-order uncertainty propagation.
    pub components: Vec<RegionCompatibilityComponent>,
    /// Root-sum-square propagated uncertainty of the weighted total distance.
    pub total_distance_standard_uncertainty: Option<f64>,
    /// Exact uncertainty convention used by this version.
    pub uncertainty_method: &'static str,
    /// Claim ceiling for a comparison without replicated patient-level design.
    pub inferential_scope: &'static str,
}

/// Validate, canonicalize, and digest a structured spatial fingerprint.
pub fn build_spatial_fingerprint(
    input: SpatialFingerprintInput,
) -> Result<SpatialFingerprint, CohortInferenceError> {
    if input.sample_id.trim().is_empty() || input.sample_id.trim() != input.sample_id {
        return Err(CohortInferenceError::InvalidInput(
            "fingerprint sample_id must be non-empty without surrounding whitespace".into(),
        ));
    }
    if input.spec_version.trim().is_empty() || input.spec_version.trim() != input.spec_version {
        return Err(CohortInferenceError::InvalidInput(
            "fingerprint spec_version must be non-empty without surrounding whitespace".into(),
        ));
    }
    if input.components.is_empty() || input.components.len() > MAXIMUM_COMPONENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "fingerprint requires 1 to {MAXIMUM_COMPONENTS} components"
        )));
    }
    let mut components = input.components;
    components.sort_by(|left, right| left.name.cmp(&right.name));
    let mut names = HashSet::with_capacity(components.len());
    let mut total_values = 0usize;
    for component in &components {
        if component.name.trim().is_empty() || component.name.trim() != component.name {
            return Err(CohortInferenceError::InvalidInput(
                "fingerprint component names must be non-empty without surrounding whitespace"
                    .into(),
            ));
        }
        if !names.insert(component.name.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate fingerprint component: {}",
                component.name
            )));
        }
        if component.axis.len() < 2
            || component.values.len() != component.axis.len()
            || component.uncertainties.len() != component.axis.len()
        {
            return Err(CohortInferenceError::InvalidInput(format!(
                "component {} requires at least two aligned axis/value/uncertainty entries",
                component.name
            )));
        }
        total_values = total_values
            .checked_add(component.axis.len())
            .ok_or_else(value_limit_error)?;
        if total_values > MAXIMUM_COMPONENT_VALUES {
            return Err(value_limit_error());
        }
        if component.axis.iter().any(|value| !value.is_finite())
            || component.axis.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(CohortInferenceError::InvalidInput(format!(
                "component {} axis must be finite and strictly increasing",
                component.name
            )));
        }
        if component.values.iter().any(|value| !value.is_finite()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "component {} values must be finite",
                component.name
            )));
        }
        if component
            .uncertainties
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(CohortInferenceError::InvalidInput(format!(
                "component {} uncertainties must be finite and nonnegative",
                component.name
            )));
        }
        if !(component.weight.is_finite() && component.weight > 0.0) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "component {} weight must be finite and positive",
                component.name
            )));
        }
    }

    let specification_digest = specification_digest(&input.spec_version, &components);
    let digest = content_digest(
        &input.sample_id,
        &input.spec_version,
        &specification_digest,
        &components,
    );
    Ok(SpatialFingerprint {
        sample_id: input.sample_id,
        spec_version: input.spec_version,
        normalization: NORMALIZATION,
        uncertainty_weighting: UNCERTAINTY_WEIGHTING,
        missing_component_policy: MISSING_COMPONENT_POLICY,
        training_transform: TRAINING_TRANSFORM,
        components,
        specification_digest,
        digest,
    })
}

/// Compare two fingerprints with the exact same specification using weighted curve L2 distance.
pub fn fingerprint_distance(
    left: &SpatialFingerprint,
    right: &SpatialFingerprint,
) -> Result<FingerprintDistanceResult, CohortInferenceError> {
    if left.specification_digest != right.specification_digest
        || left.spec_version != right.spec_version
        || left.components.len() != right.components.len()
    {
        return Err(CohortInferenceError::InvalidInput(
            "fingerprints do not share the exact specification".into(),
        ));
    }
    let mut components = Vec::with_capacity(left.components.len());
    for (left_component, right_component) in left.components.iter().zip(&right.components) {
        if left_component.name != right_component.name
            || left_component.axis != right_component.axis
            || left_component.weight.to_bits() != right_component.weight.to_bits()
        {
            return Err(CohortInferenceError::InvalidInput(
                "fingerprints do not share exact component identities, axes, and weights".into(),
            ));
        }
        let squared_integral = left_component
            .axis
            .windows(2)
            .zip(
                left_component
                    .values
                    .windows(2)
                    .zip(right_component.values.windows(2)),
            )
            .map(|(axis, (left_values, right_values))| {
                let first = left_values[0] - right_values[0];
                let second = left_values[1] - right_values[1];
                (axis[1] - axis[0]) * (first * first + second * second) / 2.0
            })
            .sum::<f64>();
        let distance = squared_integral.sqrt();
        let weighted_contribution = left_component.weight * distance;
        if !distance.is_finite() || !weighted_contribution.is_finite() {
            return Err(CohortInferenceError::NumericalFailure(format!(
                "component {} distance is non-finite",
                left_component.name
            )));
        }
        components.push(FingerprintComponentDistance {
            component: left_component.name.clone(),
            weight: left_component.weight,
            distance: if distance == 0.0 { 0.0 } else { distance },
            weighted_contribution: if weighted_contribution == 0.0 {
                0.0
            } else {
                weighted_contribution
            },
        });
    }
    let total_distance = compensated_sum(
        components
            .iter()
            .map(|component| component.weighted_contribution),
    );
    if !total_distance.is_finite() {
        return Err(CohortInferenceError::NumericalFailure(
            "fingerprint total distance is non-finite".into(),
        ));
    }
    Ok(FingerprintDistanceResult {
        specification_digest: left.specification_digest.clone(),
        left_digest: left.digest.clone(),
        right_digest: right.digest.clone(),
        components,
        total_distance: if total_distance == 0.0 {
            0.0
        } else {
            total_distance
        },
    })
}

/// Compare two within-patient regions and propagate independent endpoint standard uncertainties.
///
/// The component L2 derivatives are evaluated at the observed curves. A component
/// uncertainty is unavailable when its observed distance is zero but its endpoint
/// uncertainty is nonzero, because the L2 norm is not differentiable there. The
/// output is descriptive and does not perform population inference.
pub fn region_compatibility(
    left: &SpatialFingerprint,
    right: &SpatialFingerprint,
) -> Result<RegionCompatibilityResult, CohortInferenceError> {
    let distance = fingerprint_distance(left, right)?;
    let mut components = Vec::with_capacity(distance.components.len());
    let mut weighted_variances = Vec::with_capacity(distance.components.len());
    let mut total_available = true;

    for ((left_component, right_component), component_distance) in left
        .components
        .iter()
        .zip(&right.components)
        .zip(&distance.components)
    {
        let mut quadrature_weights = vec![0.0; left_component.axis.len()];
        for (index, axis) in left_component.axis.windows(2).enumerate() {
            let half_width = (axis[1] - axis[0]) / 2.0;
            quadrature_weights[index] += half_width;
            quadrature_weights[index + 1] += half_width;
        }
        let endpoint_variances = left_component
            .uncertainties
            .iter()
            .zip(&right_component.uncertainties)
            .map(|(left_uncertainty, right_uncertainty)| {
                left_uncertainty * left_uncertainty + right_uncertainty * right_uncertainty
            })
            .collect::<Vec<_>>();
        if endpoint_variances
            .iter()
            .any(|variance| !variance.is_finite())
        {
            return Err(CohortInferenceError::NumericalFailure(format!(
                "component {} endpoint variance is non-finite",
                left_component.name
            )));
        }

        let distance_standard_uncertainty = if component_distance.distance > 0.0 {
            let variance = compensated_sum(
                quadrature_weights
                    .iter()
                    .zip(left_component.values.iter().zip(&right_component.values))
                    .zip(&endpoint_variances)
                    .map(
                        |((quadrature_weight, (left_value, right_value)), endpoint_variance)| {
                            let derivative = quadrature_weight * (left_value - right_value)
                                / component_distance.distance;
                            derivative * derivative * endpoint_variance
                        },
                    ),
            );
            if !variance.is_finite() || variance < 0.0 {
                return Err(CohortInferenceError::NumericalFailure(format!(
                    "component {} propagated variance is invalid",
                    left_component.name
                )));
            }
            Some(variance.sqrt())
        } else if endpoint_variances.iter().all(|variance| *variance == 0.0) {
            Some(0.0)
        } else {
            None
        };

        if let Some(standard_uncertainty) = distance_standard_uncertainty {
            let weighted = left_component.weight * standard_uncertainty;
            weighted_variances.push(weighted * weighted);
        } else {
            total_available = false;
        }
        components.push(RegionCompatibilityComponent {
            component: left_component.name.clone(),
            distance: component_distance.distance,
            distance_standard_uncertainty,
        });
    }

    let total_distance_standard_uncertainty = if total_available {
        let variance = compensated_sum(weighted_variances);
        if !variance.is_finite() || variance < 0.0 {
            return Err(CohortInferenceError::NumericalFailure(
                "total propagated fingerprint variance is invalid".into(),
            ));
        }
        Some(variance.sqrt())
    } else {
        None
    };
    Ok(RegionCompatibilityResult {
        distance,
        components,
        total_distance_standard_uncertainty,
        uncertainty_method: "independent_endpoint_delta_method",
        inferential_scope: "within_patient_descriptive_only",
    })
}

fn specification_digest(version: &str, components: &[SpatialFingerprintComponent]) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "marklab.spatial_fingerprint.spec.v1");
    hash_text(&mut hasher, version);
    hash_text(&mut hasher, NORMALIZATION);
    hash_text(&mut hasher, UNCERTAINTY_WEIGHTING);
    hash_text(&mut hasher, MISSING_COMPONENT_POLICY);
    hash_text(&mut hasher, TRAINING_TRANSFORM);
    hash_usize(&mut hasher, components.len());
    for component in components {
        hash_text(&mut hasher, &component.name);
        hash_usize(&mut hasher, component.axis.len());
        for axis in &component.axis {
            hasher.update(axis.to_bits().to_be_bytes());
        }
        hasher.update(component.weight.to_bits().to_be_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn content_digest(
    sample_id: &str,
    version: &str,
    spec_digest: &str,
    components: &[SpatialFingerprintComponent],
) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "marklab.spatial_fingerprint.content.v1");
    hash_text(&mut hasher, sample_id);
    hash_text(&mut hasher, version);
    hash_text(&mut hasher, spec_digest);
    for component in components {
        hash_text(&mut hasher, &component.name);
        for value in &component.values {
            hasher.update(value.to_bits().to_be_bytes());
        }
        for uncertainty in &component.uncertainties {
            hasher.update(uncertainty.to_bits().to_be_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hash_usize(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_usize(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

fn value_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "fingerprint exceeds the {MAXIMUM_COMPONENT_VALUES}-value limit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(sample: &str, values: [f64; 2]) -> SpatialFingerprint {
        build_spatial_fingerprint(SpatialFingerprintInput {
            sample_id: sample.into(),
            spec_version: "v1".into(),
            components: vec![SpatialFingerprintComponent {
                name: "c1".into(),
                axis: vec![0.0, 1.0],
                values: values.into(),
                uncertainties: vec![0.1, 0.1],
                weight: 2.0,
            }],
        })
        .expect("fingerprint")
    }

    #[test]
    fn weighted_l2_matches_hand_oracle() {
        let left = fingerprint("left", [0.0, 0.0]);
        let right = fingerprint("right", [3.0, 4.0]);
        assert_eq!(
            left.specification_digest,
            "5de570d253bc46e4633c61a5c47cc28c837c27dbde9c283d1365dc8d3d04aa27"
        );
        assert_eq!(
            left.digest,
            "bb51c08c8dbab559ed1201c7e8ecc605b90c9922c3bc1f845c505875475fc16b"
        );
        assert_eq!(
            right.digest,
            "12b686c05acc9c83fd00c725ab867aa8712445167cb5e9588361d2027024d427"
        );
        let result = fingerprint_distance(&left, &right).expect("distance");
        assert!((result.components[0].distance - 12.5_f64.sqrt()).abs() < 1e-12);
        assert!((result.total_distance - 2.0 * 12.5_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn region_compatibility_propagates_endpoint_uncertainty() {
        let left = fingerprint("left", [0.0, 0.0]);
        let right = fingerprint("right", [3.0, 4.0]);

        let result = region_compatibility(&left, &right).expect("compatibility");

        assert!((result.distance.total_distance - 2.0 * 12.5_f64.sqrt()).abs() < 1e-12);
        assert_eq!(
            result.uncertainty_method,
            "independent_endpoint_delta_method"
        );
        assert_eq!(result.inferential_scope, "within_patient_descriptive_only");
        assert!(
            (result
                .components
                .first()
                .expect("component")
                .distance_standard_uncertainty
                .expect("component uncertainty")
                - 0.1)
                .abs()
                < 1e-12
        );
        assert!(
            (result
                .total_distance_standard_uncertainty
                .expect("total uncertainty")
                - 0.2)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn mismatched_axis_is_rejected() {
        let left = fingerprint("left", [0.0, 0.0]);
        let mut right = fingerprint("right", [3.0, 4.0]);
        right.components[0].axis[1] = 2.0;
        assert!(fingerprint_distance(&left, &right).is_err());
    }

    #[test]
    fn canonical_digests_ignore_input_component_order_and_separate_values_from_spec() {
        let component = |name: &str, values: [f64; 2]| SpatialFingerprintComponent {
            name: name.into(),
            axis: vec![0.0, 1.0],
            values: values.into(),
            uncertainties: vec![0.1, 0.1],
            weight: 1.0,
        };
        let first = build_spatial_fingerprint(SpatialFingerprintInput {
            sample_id: "sample".into(),
            spec_version: "v1".into(),
            components: vec![component("b", [2.0, 3.0]), component("a", [0.0, 1.0])],
        })
        .expect("first fingerprint");
        let reordered = build_spatial_fingerprint(SpatialFingerprintInput {
            sample_id: "sample".into(),
            spec_version: "v1".into(),
            components: vec![component("a", [0.0, 1.0]), component("b", [2.0, 3.0])],
        })
        .expect("reordered fingerprint");
        let changed = build_spatial_fingerprint(SpatialFingerprintInput {
            sample_id: "sample".into(),
            spec_version: "v1".into(),
            components: vec![component("a", [0.0, 9.0]), component("b", [2.0, 3.0])],
        })
        .expect("changed fingerprint");
        assert_eq!(first.digest, reordered.digest);
        assert_eq!(first.specification_digest, changed.specification_digest);
        assert_ne!(first.digest, changed.digest);
    }
}
