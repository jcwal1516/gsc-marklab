//! Common semantic checks for the four exact edge-corrected spatial documents.

use crate::result_vocabulary::{
    WindowCoordinateFrame, WindowCoordinateUnit, WindowMembershipPolicy,
};
use crate::{
    errors::{MarklabError, Result},
    spatial::{
        ClassicalInferenceSummary, ClassicalNullDesign, ClassicalNullModel,
        ClassicalRandomizationUnit, ClassicalWindowSummary, PairCorrelationPointStatus,
    },
    workflow_codec::unit_interval,
};

#[derive(Clone, Copy)]
pub(crate) enum Statistic {
    Kl { available: bool },
    PairCorrelation { bandwidth: f64 },
}

pub(crate) struct Analysis<'a> {
    pub(crate) isotropic: bool,
    pub(crate) statistic: Statistic,
    pub(crate) window: &'a ClassicalWindowSummary,
    pub(crate) null: &'a ClassicalNullDesign,
    pub(crate) inference: Option<&'a ClassicalInferenceSummary>,
    pub(crate) point_count: usize,
    pub(crate) maximum_radius: f64,
    pub(crate) observed_pairs: usize,
    pub(crate) total_pairs: usize,
    pub(crate) observed_evaluations: usize,
    pub(crate) total_evaluations: usize,
    pub(crate) total_work: usize,
    /// Arc membership queries or maximum overlap output vertices.
    pub(crate) auxiliary_work: usize,
    pub(crate) maximum_angles: usize,
    pub(crate) csr_draws: usize,
    pub(crate) curve_count: usize,
    /// The observed and maximum values for each of the eight declared resource ceilings.
    pub(crate) resources: [(usize, usize); 8],
}

pub(crate) struct Point {
    pub(crate) radius: f64,
    pub(crate) pairs: usize,
    pub(crate) evaluations: usize,
    pub(crate) measure: f64,
    pub(crate) weight: f64,
    /// K for a cumulative curve, g for a kernel curve.
    pub(crate) value: Option<f64>,
    pub(crate) l: Option<f64>,
    pub(crate) theoretical: f64,
    pub(crate) theoretical_l: f64,
    pub(crate) lower: Option<f64>,
    pub(crate) upper: Option<f64>,
    pub(crate) status: Option<PairCorrelationPointStatus>,
    pub(crate) eligible: bool,
}

pub(crate) fn validate_analysis(
    analysis: Analysis<'_>,
    points: impl Iterator<Item = Point>,
) -> Result<()> {
    let a = analysis;
    let name = if a.isotropic {
        "isotropic"
    } else {
        "translation"
    };
    let invalid = |reason: &str| MarklabError::Schema(format!("{name} {reason}"));
    let kernel = matches!(a.statistic, Statistic::PairCorrelation { .. });
    // Preserve the original isotropic g document's less restrictive geometry checks.
    let strict_geometry = !a.isotropic || !kernel;
    let window = a.window;
    if window.coordinate_unit != WindowCoordinateUnit::Micrometre
        || window.coordinate_frame != WindowCoordinateFrame::ExistingPatternPhysicalXy
        || window.membership_policy != WindowMembershipPolicy::ClosedWindow
        || !window.area_um2.is_finite()
        || window.area_um2 <= 0.0
        || !window.perimeter_um.is_finite()
        || window.perimeter_um <= 0.0
        || window.bounds_um.iter().any(|value| !value.is_finite())
        || window.component_count == 0
        || !valid_digest(&window.logical_digest)
        || (strict_geometry
            && (window.bounds_um[0] >= window.bounds_um[2]
                || window.bounds_um[1] >= window.bounds_um[3]
                || window.ring_count != window.component_count.saturating_add(window.hole_count)
                || window.vertex_count < window.ring_count.saturating_mul(4)))
    {
        return Err(invalid("result window is invalid"));
    }
    let minimum_radius = match a.statistic {
        Statistic::Kl { .. } => 0.0,
        Statistic::PairCorrelation { bandwidth } => bandwidth,
    };
    if a.point_count != a.null.conditioned_point_count
        || !a.maximum_radius.is_finite()
        || (strict_geometry && a.maximum_radius <= minimum_radius)
        || a.total_pairs < a.observed_pairs
        || a.total_evaluations < a.observed_evaluations
        || a.total_evaluations
            > a.total_pairs
                .saturating_mul(if a.isotropic { 2 } else { 1 })
        || a.curve_count == 0
        || a.resources
            .iter()
            .any(|(observed, maximum)| observed > maximum || (strict_geometry && *maximum == 0))
    {
        return Err(invalid("configuration or telemetry exceeds its limits"));
    }
    let segments = window.vertex_count.saturating_sub(window.ring_count);
    let expected_work = if a.isotropic && kernel {
        a.total_evaluations.saturating_mul(segments)
    } else if a.isotropic {
        segments
            .checked_mul(a.total_evaluations)
            .ok_or_else(|| invalid("segment work overflow"))?
    } else {
        segments
            .checked_mul(segments)
            .and_then(|n| n.checked_mul(a.total_evaluations))
            .ok_or_else(|| invalid("work count overflow"))?
    };
    if expected_work != a.total_work
        || (a.isotropic && !kernel && a.maximum_angles > segments.saturating_mul(2))
    {
        return Err(invalid("geometric-work telemetry differs"));
    }
    let null = a.null;
    if null.null_model != ClassicalNullModel::HomogeneousCsrConditionalOnCount
        || null.randomization_unit != ClassicalRandomizationUnit::WholeLocationPattern
        || null.simulations == 0
        || !null.alpha.is_finite()
        || null.alpha <= 0.0
        || null.alpha >= 1.0
        || (strict_geometry && (null.simulations.saturating_add(1) as f64) * null.alpha < 1.0)
    {
        return Err(invalid("null design is invalid"));
    }
    let denominator = a
        .point_count
        .checked_mul(a.point_count.saturating_sub(1))
        .ok_or_else(|| invalid("denominator overflow"))?;
    let expected_pairs = (denominator / 2)
        .checked_mul(null.simulations.saturating_add(1))
        .ok_or_else(|| invalid("total pair count overflow"))?;
    if a.observed_pairs != denominator / 2 || a.total_pairs != expected_pairs {
        return Err(invalid("pair-visit telemetry differs"));
    }
    let mut previous = (0.0, 0, 0.0, 0.0);
    let mut last_evaluations = 0;
    let mut maximum_evaluations = 0;
    let mut summed_evaluations = 0_usize;
    let mut eligible_count = 0;
    for point in points {
        if !point.radius.is_finite()
            || point.radius <= previous.0
            || point.pairs % 2 != 0
            || point.pairs > denominator
            || point.evaluations != point.pairs / if a.isotropic { 1 } else { 2 }
            || !point.measure.is_finite()
            || point.measure < 0.0
            || !point.weight.is_finite()
            || point.weight < 0.0
            || (point.pairs == 0) != (point.measure == 0.0)
            || (point.pairs == 0) != (point.weight == 0.0)
        {
            return Err(invalid("curve identity or count is invalid"));
        }
        match a.statistic {
            Statistic::Kl { available } => {
                if !same_float(
                    point.theoretical,
                    std::f64::consts::PI * point.radius * point.radius,
                ) || point.theoretical_l != point.radius
                    || point.pairs < previous.1
                    || point.measure < previous.2
                    || point.weight < previous.3
                    || (a.isotropic
                        && (point.measure > point.pairs as f64
                            || point.weight < point.pairs as f64))
                {
                    return Err(invalid("curve identity or count is invalid"));
                }
                match (available, point.value, point.l) {
                    (true, Some(k), Some(l)) => {
                        let expected = window.area_um2 * point.weight / denominator as f64;
                        if denominator == 0
                            || !k.is_finite()
                            || k < 0.0
                            || !l.is_finite()
                            || l < 0.0
                            || !same_float(k, expected)
                            || !same_float(l, (k / std::f64::consts::PI).sqrt())
                            || !nonnegative(point.lower)
                            || !nonnegative(point.upper)
                            || point.lower > point.upper
                        {
                            return Err(invalid("available K/L identity is invalid"));
                        }
                    }
                    (false, None, None) => {
                        if point.pairs != 0
                            || point.evaluations != 0
                            || point.measure != 0.0
                            || point.weight != 0.0
                            || point.lower.is_some()
                            || point.upper.is_some()
                        {
                            return Err(invalid("insufficient curve contains values"));
                        }
                    }
                    _ => return Err(invalid("curve availability differs from result status")),
                }
            }
            Statistic::PairCorrelation { bandwidth } => {
                let available = point.status == Some(PairCorrelationPointStatus::Available);
                // Older documents used null for empty support; new estimates retain zero.
                let zero_estimate = point.status
                    == Some(PairCorrelationPointStatus::NoPairsInKernelSupport)
                    && denominator > 0
                    && point.value == Some(0.0);
                if point.radius <= bandwidth
                    || point.status == Some(PairCorrelationPointStatus::NoEligibleCenters)
                    || available != (point.pairs > 0)
                    || point.value.is_some() != (available || zero_estimate)
                    || (point.eligible && point.value.is_none())
                    || point.theoretical != 1.0
                    || point.eligible != (point.lower.is_some() && point.upper.is_some())
                    || point
                        .lower
                        .is_some_and(|value| !value.is_finite() || value < 0.0)
                    || point
                        .upper
                        .is_some_and(|value| !value.is_finite() || value < 0.0)
                    || (!a.isotropic
                        && point
                            .lower
                            .zip(point.upper)
                            .is_some_and(|(lower, upper)| lower > upper))
                {
                    return Err(invalid("pair-correlation curve identity is invalid"));
                }
                if let Some(g) = point.value {
                    let expected = window.area_um2 * point.weight
                        / (std::f64::consts::TAU * point.radius * denominator as f64);
                    if denominator == 0 || !g.is_finite() || !same_float(g, expected) {
                        return Err(invalid("pair-correlation normalization differs"));
                    }
                }
                maximum_evaluations = maximum_evaluations.max(point.evaluations);
                summed_evaluations = summed_evaluations
                    .checked_add(point.evaluations)
                    .ok_or_else(|| invalid("evaluation count overflow"))?;
                eligible_count += usize::from(point.eligible);
            }
        }
        previous = (point.radius, point.pairs, point.measure, point.weight);
        last_evaluations = point.evaluations;
    }
    match a.statistic {
        Statistic::Kl { .. } => {
            if previous.0 != a.maximum_radius || last_evaluations != a.observed_evaluations {
                return Err(invalid("observed radius or correction telemetry differs"));
            }
        }
        Statistic::PairCorrelation { bandwidth } => {
            if !same_float(previous.0 + bandwidth, a.maximum_radius)
                || a.observed_evaluations < maximum_evaluations
                || a.observed_evaluations > summed_evaluations
            {
                return Err(invalid(
                    "pair-correlation observed radius or correction telemetry differs",
                ));
            }
        }
    }
    let expects_inference = match a.statistic {
        Statistic::Kl { available } => {
            eligible_count = a.curve_count;
            available
        }
        Statistic::PairCorrelation { .. } => eligible_count > 0,
    };
    match a.inference {
        Some(inference) if expects_inference => {
            if (strict_geometry && a.point_count < 2)
                || inference.simulations != null.simulations
                || inference.eligible_radius_count != eligible_count
                || !unit_interval(inference.p_global)
                || !unit_interval(inference.erl_depth)
                || !unit_interval(inference.critical_depth)
                || (strict_geometry && inference.p_global < 1.0 / (null.simulations + 1) as f64)
            {
                return Err(invalid("inference summary differs"));
            }
        }
        None if !expects_inference => {
            if !kernel && a.point_count >= 2 {
                return Err(invalid("insufficient result is inconsistent"));
            }
            if strict_geometry
                && a.point_count < 2
                && (a.csr_draws != 0
                    || a.total_pairs != 0
                    || a.total_evaluations != 0
                    || a.total_work != 0
                    || (a.isotropic && a.auxiliary_work != 0))
            {
                return Err(invalid("insufficient result is inconsistent"));
            }
        }
        _ => return Err(invalid("inference summary differs")),
    }
    Ok(())
}

pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn nonnegative(value: Option<f64>) -> bool {
    value.is_some_and(|value| value.is_finite() && value >= 0.0)
}
fn same_float(left: f64, right: f64) -> bool {
    left == right
        || (left.is_finite() && right.is_finite() && left.to_bits().abs_diff(right.to_bits()) <= 1)
}
