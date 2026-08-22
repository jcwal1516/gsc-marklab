use std::collections::{BTreeMap, BTreeSet};

use crate::{
    comparison::{
        margin_assessment::curve_margin_assessment,
        pooled_bin_difference::pooled_bin_difference_diagnostic,
    },
    output::{
        CrossInteractionCurve, CurveComparisonAvailability, CurveComparisonMethod,
        CurveComparisonResult,
    },
};

use super::axes::cross_interaction_axes_aligned;

const PREPOST_CURVE_COMPARISON_SEED: u64 = 0x7072_6570_6f73_7400;
const PREPOST_MULTIMODAL_CURVE_PERMUTATIONS: usize = 99;

#[derive(Clone, Copy)]
pub(super) struct CurveComparisonPlan {
    pub(super) permutations: usize,
    pub(super) seed: u64,
    pub(super) descriptive_margin: Option<f64>,
}

pub(super) fn base_seed() -> u64 {
    PREPOST_CURVE_COMPARISON_SEED
}

pub(super) fn append_curve_comparisons(
    tests: &mut Vec<CurveComparisonResult>,
    comparison_name: &str,
    pre_values: &[f64],
    post_values: &[f64],
    axis_alignment: Result<(), String>,
    plan: CurveComparisonPlan,
) {
    if pre_values.is_empty() && post_values.is_empty() {
        tests.push(curve_comparison_error(
            comparison_name,
            "curve_availability",
            format!(
                "{comparison_name} curve is absent in both pre/post results; pooled-bin difference and descriptive margin diagnostics were not computed"
            ),
        ));
        return;
    }
    if let Err(reason) = axis_alignment {
        tests.push(curve_comparison_error(
            comparison_name,
            "axis_alignment",
            format!(
                "{comparison_name} curve axis is not aligned: {reason}; pooled-bin difference and descriptive margin diagnostics were not computed"
            ),
        ));
        return;
    }
    if pre_values.is_empty() || pre_values.len() != post_values.len() {
        tests.push(curve_comparison_error(
            comparison_name,
            "axis_alignment",
            format!(
                "{comparison_name} curve axis is not aligned: curve lengths differ or one curve is empty; pooled-bin difference and descriptive margin diagnostics were not computed"
            ),
        ));
        return;
    }

    match pooled_bin_difference_diagnostic(
        comparison_name,
        pre_values,
        post_values,
        plan.permutations,
        plan.seed,
    ) {
        Ok(test) => tests.push(test),
        Err(err) => tests.push(curve_comparison_error(
            comparison_name,
            "max_abs_standardized_difference",
            format!("curve difference diagnostic could not be computed: {err}"),
        )),
    }
    match curve_margin_assessment(
        comparison_name,
        pre_values,
        post_values,
        plan.descriptive_margin,
    ) {
        Ok(test) => tests.push(test),
        Err(err) => tests.push(curve_comparison_error(
            comparison_name,
            "max_abs_standardized_difference",
            format!("curve margin assessment could not be computed: {err}"),
        )),
    }
}

pub(super) fn append_cross_interaction_curve_comparisons(
    tests: &mut Vec<CurveComparisonResult>,
    pre_curves: Option<&[CrossInteractionCurve]>,
    post_curves: Option<&[CrossInteractionCurve]>,
    descriptive_margin: Option<f64>,
) {
    let pre_curves = cross_curve_map(pre_curves.unwrap_or(&[]));
    let post_curves = cross_curve_map(post_curves.unwrap_or(&[]));
    let keys = pre_curves
        .keys()
        .chain(post_curves.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for key in keys {
        let comparison_name = format!("cross_interaction:{}/{}", key.0, key.1);
        match (pre_curves.get(&key), post_curves.get(&key)) {
            (Some(pre_curve), Some(post_curve)) => append_curve_comparisons(
                tests,
                &comparison_name,
                &cross_interaction_values(pre_curve),
                &cross_interaction_values(post_curve),
                cross_interaction_axes_aligned(pre_curve, post_curve),
                CurveComparisonPlan {
                    permutations: PREPOST_MULTIMODAL_CURVE_PERMUTATIONS,
                    seed: PREPOST_CURVE_COMPARISON_SEED ^ cross_curve_seed(&key),
                    descriptive_margin,
                },
            ),
            _ => tests.push(curve_comparison_error(
                &comparison_name,
                "curve_availability",
                format!(
                    "{comparison_name} curve is absent from one pre/post result; pooled-bin difference and descriptive margin diagnostics were not computed"
                ),
            )),
        }
    }
}

fn curve_comparison_error(
    comparison_name: &str,
    metric: &str,
    interpretation: String,
) -> CurveComparisonResult {
    CurveComparisonResult {
        comparison_name: comparison_name.to_owned(),
        method: CurveComparisonMethod::Unavailable,
        metric: metric.to_owned(),
        availability: CurveComparisonAvailability::InsufficientData,
        statistic: None,
        unavailable_reason: Some(interpretation.clone()),
        pooled_bin_p_value: None,
        margin: None,
        within_margin: None,
        interpretation,
    }
}

fn cross_curve_map(
    curves: &[CrossInteractionCurve],
) -> BTreeMap<(String, String), &CrossInteractionCurve> {
    curves
        .iter()
        .map(|curve| ((curve.label_a.clone(), curve.label_b.clone()), curve))
        .collect()
}

fn cross_interaction_values(curve: &CrossInteractionCurve) -> Vec<f64> {
    curve
        .points
        .iter()
        .filter_map(|point| point.value)
        .collect()
}

fn cross_curve_seed(key: &(String, String)) -> u64 {
    key.0.bytes().chain(key.1.bytes()).fold(0_u64, |acc, byte| {
        acc.wrapping_mul(1099511628211).wrapping_add(byte as u64)
    })
}
