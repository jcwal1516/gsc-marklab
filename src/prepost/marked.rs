use crate::output::{AnalysisSection, CurveComparisonResult, MarkedPatternResult, PrePostResult};

use super::{
    axes::{mark_pair_covariance_axes_aligned, spectrum_axes_aligned},
    context::ComparisonContext,
    curves::{append_curve_comparisons, base_seed, CurveComparisonPlan},
    numeric_delta, territories,
};

pub fn compare_marked_prepost(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> PrePostResult {
    let context = ComparisonContext::from_metadata(
        &pre.case_id,
        &pre.timepoint,
        &pre.protein,
        &post.case_id,
        &post.timepoint,
        &post.protein,
    );
    let territory_summary = territories::marked_summary(pre, post);
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
    let interpretation_text = context.marked_interpretation();

    PrePostResult {
        status_flags: context.status_flags,
        curve_comparisons: curve_comparisons(pre, post),
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

fn curve_comparisons(
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
        CurveComparisonPlan {
            permutations: pre
                .spectrum
                .value()
                .map_or(0, |value| value.n_permutations)
                .max(
                    post.spectrum
                        .value()
                        .map_or(0, |value| value.n_permutations),
                ),
            seed: base_seed(),
            descriptive_margin: None,
        },
    );
    append_curve_comparisons(
        &mut tests,
        "mark_pair_covariance",
        &mark_pair_covariance_values(pre),
        &mark_pair_covariance_values(post),
        mark_pair_covariance_axes_aligned(pre, post),
        CurveComparisonPlan {
            permutations: pre
                .mark_pair_covariance
                .value()
                .map_or(0, |value| value.n_permutations)
                .max(
                    post.mark_pair_covariance
                        .value()
                        .map_or(0, |value| value.n_permutations),
                ),
            seed: base_seed() ^ 0x7061_6972,
            descriptive_margin: None,
        },
    );
    tests
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
