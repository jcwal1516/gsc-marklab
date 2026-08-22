use std::collections::{BTreeMap, BTreeSet};

use crate::{
    common::stats::{mean_all_finite, median_average_even},
    comparison::{
        margin_assessment::curve_margin_assessment,
        pooled_bin_difference::pooled_bin_difference_diagnostic,
    },
    output::{
        AnalysisSection, CrossInteractionCurve, CurveComparisonAvailability, CurveComparisonResult,
        MarkedPatternResult, MultimodalResult, PrePostResult, ResidualTerritory, StatusFlag,
        TerritoryFeature, TerritoryPrePostSummary,
    },
};

// Result documents may reconstruct the same decimal axis through different
// floating-point operations. This tolerance is deliberately much smaller than
// any configured physical bin width or spectral-mode spacing.
const AXIS_ABSOLUTE_TOLERANCE: f64 = 1e-12;
const AXIS_RELATIVE_TOLERANCE: f64 = 1e-12;

const PREPOST_CURVE_COMPARISON_SEED: u64 = 0x7072_6570_6f73_7400;
const PREPOST_MULTIMODAL_CURVE_PERMUTATIONS: usize = 99;

pub fn compare_prepost(pre: &MarkedPatternResult, post: &MarkedPatternResult) -> PrePostResult {
    let mut status_flags = Vec::new();
    let anatomically_comparable = pre.case_id == post.case_id
        && pre.protein == post.protein
        && pre.timepoint.eq_ignore_ascii_case("pre")
        && post.timepoint.eq_ignore_ascii_case("post");
    if !anatomically_comparable {
        status_flags.push(StatusFlag::PrePostNotAnatomicallyComparable);
    }
    let interpretation_text = if anatomically_comparable {
        "The post-treatment section shows descriptive change in coarse-scale organization of the configured mark field compared with the pretreatment section.".into()
    } else {
        "The pre/post sections are not anatomically comparable; numeric deltas are emitted as diagnostics only.".into()
    };

    let territory_summary = territory_prepost_summary(pre, post);
    let delta_territory_count = territory_summary
        .value()
        .map(|summary| summary.delta_count)
        .map_or_else(
            || AnalysisSection::InsufficientData {
                reason: "multiscale residual territories are unavailable in one or both results"
                    .into(),
            },
            AnalysisSection::available,
        );
    let pre_spectrum = pre.spectrum.value();
    let post_spectrum = post.spectrum.value();
    let pre_anisotropy = pre.anisotropy.value();
    let post_anisotropy = post.anisotropy.value();
    let pre_multiscale_residual = pre.multiscale_residual.value();
    let post_multiscale_residual = post.multiscale_residual.value();

    PrePostResult {
        status_flags,
        curve_comparisons: prepost_curve_comparisons(pre, post),
        delta_xi_um: numeric_delta(
            pre_spectrum.and_then(|value| value.xi_um),
            post_spectrum.and_then(|value| value.xi_um),
            "xi_um is unavailable in one or both results",
        ),
        delta_low_k_excess: numeric_delta(
            pre_spectrum.map(|value| value.low_k_excess),
            post_spectrum.map(|value| value.low_k_excess),
            "spectrum is unavailable in one or both results",
        ),
        delta_alpha: numeric_delta(
            pre_spectrum.and_then(|value| value.alpha),
            post_spectrum.and_then(|value| value.alpha),
            "fitted low-k exponent is unavailable in one or both results",
        ),
        delta_anisotropy_index: numeric_delta(
            pre_anisotropy.map(|value| value.index),
            post_anisotropy.map(|value| value.index),
            "anisotropy is unavailable in one or both results",
        ),
        delta_block_mean_variance_fraction: numeric_delta(
            pre_multiscale_residual.map(|value| value.block_mean_variance_fraction),
            post_multiscale_residual.map(|value| value.block_mean_variance_fraction),
            "multiscale residual analysis is unavailable in one or both results",
        ),
        delta_territory_count,
        territory_summary,
        interpretation_text,
    }
}

pub fn compare_multimodal_prepost(
    pre: &MultimodalResult,
    post: &MultimodalResult,
) -> PrePostResult {
    let mut status_flags = Vec::new();
    let anatomically_comparable = pre.case_id == post.case_id
        && pre.protein == post.protein
        && pre.timepoint.eq_ignore_ascii_case("pre")
        && post.timepoint.eq_ignore_ascii_case("post");
    if !anatomically_comparable {
        status_flags.push(StatusFlag::PrePostNotAnatomicallyComparable);
    }

    let territory_summary = territory_prepost_summary_from_slices(
        pre.neighborhood_territories.value().map(Vec::as_slice),
        post.neighborhood_territories.value().map(Vec::as_slice),
        "neighborhood territories are unavailable in one or both results",
    );
    let delta_territory_count = territory_summary
        .value()
        .map(|summary| AnalysisSection::available(summary.delta_count))
        .unwrap_or_else(|| AnalysisSection::InsufficientData {
            reason: "neighborhood territories are unavailable in one or both results".into(),
        });
    let mut curve_comparisons = Vec::new();
    append_cross_interaction_curve_comparisons_for_sections(
        &mut curve_comparisons,
        pre.cross_interaction_curves.value().map(Vec::as_slice),
        post.cross_interaction_curves.value().map(Vec::as_slice),
    );

    PrePostResult {
        status_flags,
        curve_comparisons,
        delta_xi_um: AnalysisSection::NotApplicable,
        delta_low_k_excess: AnalysisSection::NotApplicable,
        delta_alpha: AnalysisSection::NotApplicable,
        delta_anisotropy_index: AnalysisSection::NotApplicable,
        delta_block_mean_variance_fraction: AnalysisSection::NotApplicable,
        delta_territory_count,
        territory_summary,
        interpretation_text: if anatomically_comparable {
            "The post-treatment section shows descriptive change in multimodal neighborhood organization compared with the pretreatment section.".into()
        } else {
            "The pre/post multimodal sections are not anatomically comparable; numeric deltas are emitted as diagnostics only.".into()
        },
    }
}

fn numeric_delta(pre: Option<f64>, post: Option<f64>, reason: &str) -> AnalysisSection<f64> {
    match (pre, post) {
        (Some(pre), Some(post)) if pre.is_finite() && post.is_finite() => {
            AnalysisSection::available(post - pre)
        }
        _ => AnalysisSection::InsufficientData {
            reason: reason.into(),
        },
    }
}

fn prepost_curve_comparisons(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> Vec<CurveComparisonResult> {
    let mut tests = Vec::new();

    append_curve_comparisons(
        &mut tests,
        "spectrum",
        &spectrum_values(pre),
        &spectrum_values(post),
        spectrum_axes_aligned(pre, post),
        pre.spectrum
            .value()
            .map_or(0, |value| value.n_permutations)
            .max(
                post.spectrum
                    .value()
                    .map_or(0, |value| value.n_permutations),
            ),
        PREPOST_CURVE_COMPARISON_SEED,
    );
    append_curve_comparisons(
        &mut tests,
        "mark_pair_covariance",
        &mark_pair_covariance_values(pre),
        &mark_pair_covariance_values(post),
        mark_pair_covariance_axes_aligned(pre, post),
        pre.mark_pair_covariance
            .value()
            .map_or(0, |value| value.n_permutations)
            .max(
                post.mark_pair_covariance
                    .value()
                    .map_or(0, |value| value.n_permutations),
            ),
        PREPOST_CURVE_COMPARISON_SEED ^ 0x7061_6972,
    );
    append_cross_interaction_curve_comparisons(&mut tests, pre, post);

    tests
}

fn append_curve_comparisons(
    tests: &mut Vec<CurveComparisonResult>,
    comparison_name: &str,
    pre_values: &[f64],
    post_values: &[f64],
    axis_alignment: Result<(), String>,
    permutations: usize,
    seed: u64,
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
        permutations,
        seed,
    ) {
        Ok(test) => tests.push(test),
        Err(err) => tests.push(curve_comparison_error(
            comparison_name,
            "max_abs_standardized_difference",
            format!("curve difference diagnostic could not be computed: {err}"),
        )),
    }
    match curve_margin_assessment(comparison_name, pre_values, post_values, None) {
        Ok(test) => tests.push(test),
        Err(err) => tests.push(curve_comparison_error(
            comparison_name,
            "max_abs_standardized_difference",
            format!("curve margin assessment could not be computed: {err}"),
        )),
    }
}

fn curve_comparison_error(
    comparison_name: &str,
    metric: &str,
    interpretation: String,
) -> CurveComparisonResult {
    CurveComparisonResult {
        comparison_name: comparison_name.to_owned(),
        method: crate::output::CurveComparisonMethod::Unavailable,
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

fn spectrum_values(result: &MarkedPatternResult) -> Vec<f64> {
    result
        .spectrum_curve
        .iter()
        .map(|point| point.whitened_power)
        .collect()
}

fn mark_pair_covariance_values(result: &MarkedPatternResult) -> Vec<f64> {
    result
        .mark_pair_covariance_curve
        .iter()
        .filter_map(|point| point.covariance)
        .collect()
}

fn append_cross_interaction_curve_comparisons(
    tests: &mut Vec<CurveComparisonResult>,
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) {
    append_cross_interaction_curve_comparisons_for_sections(
        tests,
        pre.cross_interaction_curves.value().map(Vec::as_slice),
        post.cross_interaction_curves.value().map(Vec::as_slice),
    );
}

fn append_cross_interaction_curve_comparisons_for_sections(
    tests: &mut Vec<CurveComparisonResult>,
    pre_curves: Option<&[CrossInteractionCurve]>,
    post_curves: Option<&[CrossInteractionCurve]>,
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
                PREPOST_MULTIMODAL_CURVE_PERMUTATIONS,
                PREPOST_CURVE_COMPARISON_SEED ^ cross_curve_seed(&key),
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

fn cross_interaction_axes_aligned(
    pre: &CrossInteractionCurve,
    post: &CrossInteractionCurve,
) -> Result<(), String> {
    if pre.points.len() != post.points.len() {
        return Err(format!(
            "cross-interaction bin counts differ: {} vs {}",
            pre.points.len(),
            post.points.len()
        ));
    }

    for (index, (pre_point, post_point)) in pre.points.iter().zip(&post.points).enumerate() {
        if !pre_point.r_min_um.is_finite()
            || !pre_point.r_max_um.is_finite()
            || !post_point.r_min_um.is_finite()
            || !post_point.r_max_um.is_finite()
        {
            return Err(format!(
                "cross-interaction axis contains non-finite bin edge at index {index}"
            ));
        }
        if !axis_values_match(pre_point.r_min_um, post_point.r_min_um)
            || !axis_values_match(pre_point.r_max_um, post_point.r_max_um)
        {
            return Err(format!(
                "cross-interaction axis differs at index {index}: [{}, {}) vs [{}, {})",
                pre_point.r_min_um, pre_point.r_max_um, post_point.r_min_um, post_point.r_max_um
            ));
        }
        if pre_point.value.is_some() != post_point.value.is_some() {
            return Err(format!(
                "cross-interaction availability differs at index {index}"
            ));
        }
    }

    Ok(())
}

fn cross_curve_seed(key: &(String, String)) -> u64 {
    key.0.bytes().chain(key.1.bytes()).fold(0_u64, |acc, byte| {
        acc.wrapping_mul(1099511628211).wrapping_add(byte as u64)
    })
}

fn territory_prepost_summary(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> AnalysisSection<TerritoryPrePostSummary> {
    territory_prepost_summary_from_slices(
        pre.residual_territories.value().map(Vec::as_slice),
        post.residual_territories.value().map(Vec::as_slice),
        "multiscale residual territories are unavailable in one or both results",
    )
}

fn territory_prepost_summary_from_slices<T: TerritorySummaryView>(
    pre_territories: Option<&[T]>,
    post_territories: Option<&[T]>,
    unavailable_reason: &str,
) -> AnalysisSection<TerritoryPrePostSummary> {
    let (Some(pre_territories), Some(post_territories)) = (pre_territories, post_territories)
    else {
        return AnalysisSection::InsufficientData {
            reason: unavailable_reason.into(),
        };
    };
    let pre_count = pre_territories.len();
    let post_count = post_territories.len();
    AnalysisSection::available(TerritoryPrePostSummary {
        pre_count,
        post_count,
        delta_count: post_count as isize - pre_count as isize,
        delta_mean_radius_um: numeric_delta(
            mean_territory_radius(pre_territories),
            mean_territory_radius(post_territories),
            "mean territory radius is undefined because one result has no territories",
        ),
        delta_median_radius_um: numeric_delta(
            median_territory_radius(pre_territories),
            median_territory_radius(post_territories),
            "median territory radius is undefined because one result has no territories",
        ),
        delta_mean_supporting_cells: numeric_delta(
            mean_supporting_cells(pre_territories),
            mean_supporting_cells(post_territories),
            "mean supporting-cell count is undefined because one result has no territories",
        ),
        delta_median_supporting_cells: numeric_delta(
            median_supporting_cells(pre_territories),
            median_supporting_cells(post_territories),
            "median supporting-cell count is undefined because one result has no territories",
        ),
        new_domain_count: unmatched_domain_count(post_territories, pre_territories),
        lost_domain_count: unmatched_domain_count(pre_territories, post_territories),
    })
}

fn mean_territory_radius(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    mean_all_finite(territories.iter().map(TerritorySummaryView::radius_um))
}

fn median_territory_radius(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    let mut values = territories
        .iter()
        .map(TerritorySummaryView::radius_um)
        .collect::<Vec<_>>();
    median_average_even(&mut values)
}

fn mean_supporting_cells(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    mean_all_finite(
        territories
            .iter()
            .map(|territory| territory.supporting_cells() as f64),
    )
}

fn median_supporting_cells(territories: &[impl TerritorySummaryView]) -> Option<f64> {
    let mut values = territories
        .iter()
        .map(|territory| territory.supporting_cells() as f64)
        .collect::<Vec<_>>();
    median_average_even(&mut values)
}

fn unmatched_domain_count<T: TerritorySummaryView>(query: &[T], reference: &[T]) -> usize {
    query
        .iter()
        .filter(|territory| {
            !reference
                .iter()
                .any(|candidate| domains_match(*territory, candidate))
        })
        .count()
}

fn domains_match(left: &impl TerritorySummaryView, right: &impl TerritorySummaryView) -> bool {
    let dx = left.center_x_um() - right.center_x_um();
    let dy = left.center_y_um() - right.center_y_um();
    let tolerance = left.radius_um().max(right.radius_um());
    dx.hypot(dy) <= tolerance
}

trait TerritorySummaryView {
    fn center_x_um(&self) -> f64;
    fn center_y_um(&self) -> f64;
    fn radius_um(&self) -> f64;
    fn supporting_cells(&self) -> usize;
}

impl TerritorySummaryView for ResidualTerritory {
    fn center_x_um(&self) -> f64 {
        self.center_x_um
    }

    fn center_y_um(&self) -> f64 {
        self.center_y_um
    }

    fn radius_um(&self) -> f64 {
        self.radius_um
    }

    fn supporting_cells(&self) -> usize {
        self.supporting_marked_cells
    }
}

impl TerritorySummaryView for TerritoryFeature {
    fn center_x_um(&self) -> f64 {
        self.center_x_um
    }

    fn center_y_um(&self) -> f64 {
        self.center_y_um
    }

    fn radius_um(&self) -> f64 {
        self.radius_um
    }

    fn supporting_cells(&self) -> usize {
        self.supporting_cells
    }
}

fn spectrum_axes_aligned(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> Result<(), String> {
    if pre.spectrum_curve.len() != post.spectrum_curve.len() {
        return Err(format!(
            "spectrum k-axis lengths differ: {} vs {}",
            pre.spectrum_curve.len(),
            post.spectrum_curve.len()
        ));
    }

    for (index, (pre_point, post_point)) in pre
        .spectrum_curve
        .iter()
        .zip(&post.spectrum_curve)
        .enumerate()
    {
        if !pre_point.k.is_finite() || !post_point.k.is_finite() {
            return Err(format!(
                "spectrum k-axis contains non-finite value at index {index}"
            ));
        }
        if !axis_values_match(pre_point.k, post_point.k) {
            return Err(format!(
                "spectrum k-axis differs at index {index}: {} vs {}",
                pre_point.k, post_point.k
            ));
        }
    }

    Ok(())
}

fn mark_pair_covariance_axes_aligned(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> Result<(), String> {
    if pre.mark_pair_covariance_curve.len() != post.mark_pair_covariance_curve.len() {
        return Err(format!(
            "mark-pair-covariance bin counts differ: {} vs {}",
            pre.mark_pair_covariance_curve.len(),
            post.mark_pair_covariance_curve.len()
        ));
    }

    for (index, (pre_point, post_point)) in pre
        .mark_pair_covariance_curve
        .iter()
        .zip(&post.mark_pair_covariance_curve)
        .enumerate()
    {
        if !pre_point.r_min_um.is_finite()
            || !pre_point.r_max_um.is_finite()
            || !post_point.r_min_um.is_finite()
            || !post_point.r_max_um.is_finite()
        {
            return Err(format!(
                "mark-pair-covariance axis contains non-finite bin edge at index {index}"
            ));
        }
        if !axis_values_match(pre_point.r_min_um, post_point.r_min_um)
            || !axis_values_match(pre_point.r_max_um, post_point.r_max_um)
        {
            return Err(format!(
                "mark-pair-covariance axis differs at index {index}: [{}, {}) vs [{}, {})",
                pre_point.r_min_um, pre_point.r_max_um, post_point.r_min_um, post_point.r_max_um
            ));
        }
        if pre_point.covariance.is_some() != post_point.covariance.is_some() {
            return Err(format!(
                "mark-pair-covariance availability differs at index {index}"
            ));
        }
    }

    Ok(())
}

fn axis_values_match(left: f64, right: f64) -> bool {
    let tolerance = AXIS_ABSOLUTE_TOLERANCE + AXIS_RELATIVE_TOLERANCE * left.abs().max(right.abs());
    (left - right).abs() <= tolerance
}
