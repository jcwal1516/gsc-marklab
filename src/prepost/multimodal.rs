use crate::output::{AnalysisSection, MultimodalResult, PrePostResult};

use super::{
    context::ComparisonContext, curves::append_cross_interaction_curve_comparisons, territories,
};

/// Compare two multimodal results without a descriptive cross-curve margin.
///
/// Cross-interaction and territory changes use the same typed availability and
/// axis-alignment policy as the CLI. With no prespecified margin,
/// `within_margin` remains unavailable.
pub fn compare_multimodal_prepost(
    pre: &MultimodalResult,
    post: &MultimodalResult,
) -> PrePostResult {
    compare_multimodal_prepost_with_margin(pre, post, None)
}

/// Compare two multimodal results with an optional descriptive cross-curve margin.
///
/// The margin checks a maximum standardized curve distance. It is not an
/// inferential equivalence test. Invalid or incomparable curves produce typed
/// insufficient-data rows rather than numeric sentinels.
pub fn compare_multimodal_prepost_with_margin(
    pre: &MultimodalResult,
    post: &MultimodalResult,
    cross_interaction_margin: Option<f64>,
) -> PrePostResult {
    let context = ComparisonContext::from_metadata(
        &pre.case_id,
        &pre.timepoint,
        &pre.protein,
        &post.case_id,
        &post.timepoint,
        &post.protein,
    );
    let territory_summary = territories::multimodal_summary(pre, post);
    let delta_territory_count = territory_summary
        .value()
        .map(|summary| AnalysisSection::available(summary.delta_count))
        .unwrap_or_else(|| AnalysisSection::InsufficientData {
            reason: "neighborhood territories are unavailable in one or both results".into(),
        });
    let mut curve_comparisons = Vec::new();
    append_cross_interaction_curve_comparisons(
        &mut curve_comparisons,
        pre.cross_interaction_curves.value().map(Vec::as_slice),
        post.cross_interaction_curves.value().map(Vec::as_slice),
        cross_interaction_margin,
    );
    let interpretation_text = context.multimodal_interpretation();

    PrePostResult {
        status_flags: context.status_flags,
        curve_comparisons,
        delta_xi_um: AnalysisSection::NotApplicable,
        delta_low_k_excess: AnalysisSection::NotApplicable,
        delta_alpha: AnalysisSection::NotApplicable,
        delta_anisotropy_index: AnalysisSection::NotApplicable,
        delta_block_mean_variance_fraction: AnalysisSection::NotApplicable,
        delta_territory_count,
        territory_summary,
        interpretation_text,
    }
}
