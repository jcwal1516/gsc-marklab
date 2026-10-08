use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ObservationWindow2D;

/// One observed event with a fixed log-linear predictor value.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppEvent {
    pub event_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub covariate: f64,
    pub offset: f64,
}

/// One positive-area quadrature node in an exact observation window.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppQuadratureNode {
    pub node_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub weight_um2: f64,
    pub covariate: f64,
    pub offset: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct ArbitraryWindowIppLimits {
    pub maximum_events: usize,
    pub maximum_quadrature_nodes: usize,
    pub maximum_work: usize,
    pub maximum_retained_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct ArbitraryWindowIppSpec {
    pub events: Vec<ArbitraryWindowIppEvent>,
    pub quadrature: Vec<ArbitraryWindowIppQuadratureNode>,
    pub intercept: f64,
    pub coefficient: f64,
    pub limits: ArbitraryWindowIppLimits,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppWindowSummary {
    pub area_um2: f64,
    pub perimeter_um: f64,
    pub bounds_um: [f64; 4],
    pub component_count: usize,
    pub hole_count: usize,
    pub ring_count: usize,
    pub vertex_count: usize,
    pub logical_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppLikelihoodResult {
    pub format: String,
    pub version: u32,
    pub window: ArbitraryWindowIppWindowSummary,
    pub events_digest: String,
    pub quadrature_digest: String,
    pub event_count: usize,
    pub quadrature_node_count: usize,
    pub quadrature_weight_um2: f64,
    pub quadrature_area_absolute_error_um2: f64,
    pub intercept: f64,
    pub coefficient: f64,
    pub event_term: f64,
    pub integral_term: f64,
    pub log_likelihood: f64,
    pub declared_work: usize,
    pub retained_bytes: usize,
    pub statistical_unit: String,
    pub null_model: String,
    pub assumptions: Vec<String>,
    pub finite_result_policy: String,
    pub claim_status: String,
}

#[derive(Debug, Error)]
pub enum ArbitraryWindowIppError {
    #[error("invalid arbitrary-window IPP input: {0}")]
    Invalid(String),
    #[error("arbitrary-window IPP numerical failure: {0}")]
    Numerical(String),
}

pub fn arbitrary_window_ipp_log_likelihood(
    window: &ObservationWindow2D,
    mut spec: ArbitraryWindowIppSpec,
) -> Result<ArbitraryWindowIppLikelihoodResult, ArbitraryWindowIppError> {
    validate_limits(&spec)?;
    spec.events
        .sort_by(|left, right| left.event_id.cmp(&right.event_id));
    spec.quadrature
        .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    validate_events(window, &spec.events)?;
    validate_quadrature(window, &spec.quadrature)?;

    let declared_work = spec
        .events
        .len()
        .checked_add(spec.quadrature.len())
        .ok_or_else(|| ArbitraryWindowIppError::Invalid("work size overflows".into()))?;
    if declared_work > spec.limits.maximum_work {
        return Err(ArbitraryWindowIppError::Invalid(format!(
            "declared work exceeds maximum_work: {declared_work} > {}",
            spec.limits.maximum_work
        )));
    }
    let retained_bytes = retained_bytes(&spec)?;
    if retained_bytes > spec.limits.maximum_retained_bytes {
        return Err(ArbitraryWindowIppError::Invalid(format!(
            "retained memory exceeds maximum_retained_bytes: {retained_bytes} > {}",
            spec.limits.maximum_retained_bytes
        )));
    }

    let quadrature_weight_um2 = stable_sum(spec.quadrature.iter().map(|node| node.weight_um2))?;
    let window_area_um2 = window.area_um2();
    let quadrature_area_absolute_error_um2 = (quadrature_weight_um2 - window_area_um2).abs();
    let area_tolerance = window_area_um2.max(1.0) * 1e-10;
    if quadrature_area_absolute_error_um2 > area_tolerance {
        return Err(ArbitraryWindowIppError::Invalid(format!(
            "quadrature weights do not sum to exact window area within tolerance: error={quadrature_area_absolute_error_um2}, tolerance={area_tolerance}"
        )));
    }

    let event_term = stable_sum(
        spec.events
            .iter()
            .map(|event| spec.intercept + spec.coefficient * event.covariate + event.offset),
    )?;
    let integral_term = stable_sum(spec.quadrature.iter().map(|node| {
        let predictor = spec.intercept + spec.coefficient * node.covariate + node.offset;
        node.weight_um2 * predictor.exp()
    }))?;
    let log_likelihood = event_term - integral_term;
    if !event_term.is_finite() || !integral_term.is_finite() || !log_likelihood.is_finite() {
        return Err(ArbitraryWindowIppError::Numerical(
            "likelihood terms are non-finite".into(),
        ));
    }
    let descriptor = window.descriptor();
    let events_digest = ContentDigest::from_bytes(
        &serde_json::to_vec(&spec.events)
            .map_err(|error| ArbitraryWindowIppError::Invalid(error.to_string()))?,
    );
    let quadrature_digest = ContentDigest::from_bytes(
        &serde_json::to_vec(&spec.quadrature)
            .map_err(|error| ArbitraryWindowIppError::Invalid(error.to_string()))?,
    );
    Ok(ArbitraryWindowIppLikelihoodResult {
        format: "marklab.arbitrary_window_ipp_likelihood".into(),
        version: 1,
        window: ArbitraryWindowIppWindowSummary {
            area_um2: descriptor.area_um2,
            perimeter_um: descriptor.perimeter_um,
            bounds_um: descriptor.bounds_um,
            component_count: descriptor.component_count,
            hole_count: descriptor.hole_count,
            ring_count: descriptor.ring_count,
            vertex_count: descriptor.vertex_count,
            logical_digest: descriptor.logical_digest.to_string(),
        },
        events_digest: events_digest.to_string(),
        quadrature_digest: quadrature_digest.to_string(),
        event_count: spec.events.len(),
        quadrature_node_count: spec.quadrature.len(),
        quadrature_weight_um2,
        quadrature_area_absolute_error_um2,
        intercept: spec.intercept,
        coefficient: spec.coefficient,
        event_term,
        integral_term,
        log_likelihood,
        declared_work,
        retained_bytes,
        statistical_unit: "one_observed_point_pattern".into(),
        null_model: "none_fixed_parameter_likelihood_evaluation".into(),
        assumptions: vec![
            "events_are_one_realization_in_the_exact_observation_window".into(),
            "supplied_covariate_and_offset_are_fixed_and_evaluated_at_events_and_quadrature_nodes"
                .into(),
            "positive_quadrature_weights_partition_the_exact_window_area".into(),
            "conditional_on_the_fixed_intensity_events_follow_an_inhomogeneous_poisson_process"
                .into(),
        ],
        finite_result_policy: "reject_non_finite".into(),
        claim_status: "experimental_fixed_likelihood_no_population_inference".into(),
    })
}

impl ArbitraryWindowIppLikelihoodResult {
    pub fn validate_for_spec(
        &self,
        window: &ObservationWindow2D,
        spec: &ArbitraryWindowIppSpec,
    ) -> Result<(), ArbitraryWindowIppError> {
        validate_limits(spec)?;
        let mut events = spec.events.clone();
        let mut quadrature = spec.quadrature.clone();
        events.sort_by(|left, right| left.event_id.cmp(&right.event_id));
        quadrature.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        validate_events(window, &events)?;
        validate_quadrature(window, &quadrature)?;
        let events_digest = digest(&events)?;
        let quadrature_digest = digest(&quadrature)?;
        let declared_work = events
            .len()
            .checked_add(quadrature.len())
            .ok_or_else(|| ArbitraryWindowIppError::Invalid("work size overflows".into()))?;
        let retained_bytes = retained_bytes(spec)?;
        let descriptor = window.descriptor();
        if self.format != "marklab.arbitrary_window_ipp_likelihood"
            || self.version != 1
            || self.window.area_um2.to_bits() != descriptor.area_um2.to_bits()
            || self.window.perimeter_um.to_bits() != descriptor.perimeter_um.to_bits()
            || self.window.bounds_um.map(f64::to_bits) != descriptor.bounds_um.map(f64::to_bits)
            || self.window.component_count != descriptor.component_count
            || self.window.hole_count != descriptor.hole_count
            || self.window.ring_count != descriptor.ring_count
            || self.window.vertex_count != descriptor.vertex_count
            || self.window.logical_digest != descriptor.logical_digest.to_string()
            || self.events_digest != events_digest
            || self.quadrature_digest != quadrature_digest
            || self.event_count != events.len()
            || self.quadrature_node_count != quadrature.len()
            || self.intercept.to_bits() != spec.intercept.to_bits()
            || self.coefficient.to_bits() != spec.coefficient.to_bits()
            || self.declared_work != declared_work
            || self.retained_bytes != retained_bytes
            || self.statistical_unit != "one_observed_point_pattern"
            || self.null_model != "none_fixed_parameter_likelihood_evaluation"
            || self.finite_result_policy != "reject_non_finite"
            || self.claim_status != "experimental_fixed_likelihood_no_population_inference"
            || ![
                self.quadrature_weight_um2,
                self.quadrature_area_absolute_error_um2,
                self.event_term,
                self.integral_term,
                self.log_likelihood,
            ]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(ArbitraryWindowIppError::Invalid(
                "arbitrary-window IPP result identity or finite contract differs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_limits(spec: &ArbitraryWindowIppSpec) -> Result<(), ArbitraryWindowIppError> {
    if spec.events.is_empty()
        || spec.quadrature.is_empty()
        || spec.events.len() > spec.limits.maximum_events
        || spec.quadrature.len() > spec.limits.maximum_quadrature_nodes
        || spec.limits.maximum_events == 0
        || spec.limits.maximum_events > 1_000_000
        || spec.limits.maximum_quadrature_nodes == 0
        || spec.limits.maximum_quadrature_nodes > 1_000_000
        || spec.limits.maximum_work == 0
        || spec.limits.maximum_work > 2_000_000
        || spec.limits.maximum_retained_bytes == 0
        || spec.limits.maximum_retained_bytes > 1024 * 1024 * 1024
        || !spec.intercept.is_finite()
        || !spec.coefficient.is_finite()
    {
        return Err(ArbitraryWindowIppError::Invalid(
            "events, quadrature, coefficients, or resource limits are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_events(
    window: &ObservationWindow2D,
    events: &[ArbitraryWindowIppEvent],
) -> Result<(), ArbitraryWindowIppError> {
    let mut previous = None;
    for event in events {
        if event.event_id.trim().is_empty()
            || event.event_id.trim() != event.event_id
            || previous.is_some_and(|value: &str| value == event.event_id)
            || ![event.x_um, event.y_um, event.covariate, event.offset]
                .into_iter()
                .all(f64::is_finite)
            || !window.contains(event.x_um, event.y_um)
        {
            return Err(ArbitraryWindowIppError::Invalid(
                "events require unique exact IDs and finite in-window values".into(),
            ));
        }
        previous = Some(event.event_id.as_str());
    }
    Ok(())
}

fn validate_quadrature(
    window: &ObservationWindow2D,
    nodes: &[ArbitraryWindowIppQuadratureNode],
) -> Result<(), ArbitraryWindowIppError> {
    let mut previous = None;
    for node in nodes {
        if node.node_id.trim().is_empty()
            || node.node_id.trim() != node.node_id
            || previous.is_some_and(|value: &str| value == node.node_id)
            || ![
                node.x_um,
                node.y_um,
                node.weight_um2,
                node.covariate,
                node.offset,
            ]
            .into_iter()
            .all(f64::is_finite)
            || node.weight_um2 <= 0.0
            || !window.contains(node.x_um, node.y_um)
        {
            return Err(ArbitraryWindowIppError::Invalid(
                "quadrature requires unique exact IDs, positive finite weights, and finite in-window values"
                    .into(),
            ));
        }
        previous = Some(node.node_id.as_str());
    }
    Ok(())
}

fn retained_bytes(spec: &ArbitraryWindowIppSpec) -> Result<usize, ArbitraryWindowIppError> {
    spec.events
        .len()
        .checked_mul(std::mem::size_of::<ArbitraryWindowIppEvent>())
        .and_then(|bytes| {
            spec.quadrature
                .len()
                .checked_mul(std::mem::size_of::<ArbitraryWindowIppQuadratureNode>())
                .and_then(|quadrature| bytes.checked_add(quadrature))
        })
        .and_then(|bytes| {
            spec.events
                .iter()
                .map(|event| event.event_id.len())
                .chain(spec.quadrature.iter().map(|node| node.node_id.len()))
                .try_fold(bytes, usize::checked_add)
        })
        .ok_or_else(|| ArbitraryWindowIppError::Invalid("retained memory overflows".into()))
}

fn digest<T: Serialize>(value: &T) -> Result<String, ArbitraryWindowIppError> {
    serde_json::to_vec(value)
        .map(|bytes| ContentDigest::from_bytes(&bytes).to_string())
        .map_err(|error| ArbitraryWindowIppError::Invalid(error.to_string()))
}

fn stable_sum(values: impl Iterator<Item = f64>) -> Result<f64, ArbitraryWindowIppError> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        if !value.is_finite() {
            return Err(ArbitraryWindowIppError::Numerical(
                "summand is non-finite".into(),
            ));
        }
        let next = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
    }
    let result = sum + correction;
    if result.is_finite() {
        Ok(result)
    } else {
        Err(ArbitraryWindowIppError::Numerical(
            "compensated sum is non-finite".into(),
        ))
    }
}
