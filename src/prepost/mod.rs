mod axes;
mod context;
mod curves;
mod marked;
mod multimodal;
mod territories;

pub use marked::compare_marked_prepost;
pub use multimodal::{compare_multimodal_prepost, compare_multimodal_prepost_with_margin};

use crate::output::AnalysisSection;

pub(super) fn numeric_delta(
    pre: Option<f64>,
    post: Option<f64>,
    reason: &str,
) -> AnalysisSection<f64> {
    match (pre, post) {
        (Some(pre), Some(post)) if pre.is_finite() && post.is_finite() => {
            AnalysisSection::available(post - pre)
        }
        _ => AnalysisSection::InsufficientData {
            reason: reason.into(),
        },
    }
}

#[cfg(test)]
mod tests;
