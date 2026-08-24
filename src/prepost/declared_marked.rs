use thiserror::Error;

use crate::{
    scalar_mark::{BinaryMarkOrigin, DeclaredMarkUse, DeclaredScalarIdentity},
    workflow::DeclaredMarkedAnalysisResult,
    PrePostResult,
};

use super::marked::compare_marked_prepost;

/// Runtime-only declared context around one unchanged marked pre/post result.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredMarkedPrePostResult<'a> {
    /// Exact compatibility pre/post result produced by the existing comparison.
    pub result: PrePostResult,
    /// Pre-analysis scalar declarations, provenance identities, and endpoint routing.
    pub pre_mark_use: DeclaredMarkUse,
    /// Post-analysis scalar declarations, provenance identities, and endpoint routing.
    pub post_mark_use: DeclaredMarkUse,
    /// Exact pre-analysis row, slide, frame, and declaration identity.
    pub pre_scalar_identity: DeclaredScalarIdentity,
    /// Exact post-analysis row, slide, frame, and declaration identity.
    pub post_scalar_identity: DeclaredScalarIdentity,
    /// Exact pre-analysis timepoint metadata retained only in this runtime wrapper.
    pub pre_timepoint: &'a str,
    /// Exact post-analysis timepoint metadata retained only in this runtime wrapper.
    pub post_timepoint: &'a str,
}

/// Semantically incompatible declared marked-analysis outputs.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DeclaredMarkedPrePostError {
    /// The pre runtime wrapper's available result/declaration bindings disagree.
    #[error("declared pre/post pre-analysis runtime bindings disagree")]
    InvalidPreRuntimeBinding,
    /// The post runtime wrapper's available result/declaration bindings disagree.
    #[error("declared pre/post post-analysis runtime bindings disagree")]
    InvalidPostRuntimeBinding,
    /// The stable binary mark IDs differ.
    #[error("declared pre/post binary mark IDs differ")]
    BinaryMarkIdMismatch,
    /// The binary display labels differ.
    #[error("declared pre/post binary mark labels differ")]
    BinaryMarkLabelMismatch,
    /// A binary or probability measurement status differs.
    #[error("declared pre/post measurement statuses differ")]
    MeasurementStatusMismatch,
    /// Structure-factor or other-endpoint value routing differs.
    #[error("declared pre/post endpoint routing differs")]
    EndpointRoutingMismatch,
    /// The stable probability mark IDs differ.
    #[error("declared pre/post probability mark IDs differ")]
    ProbabilityMarkIdMismatch,
    /// Independent/thresholded semantics, comparator, source ID, or threshold bits differ.
    #[error("declared pre/post binary mark origins differ")]
    BinaryOriginMismatch,
}

/// Compare two semantically matched declared marked scheduler outputs.
///
/// Ordered CellIds, rows, slides, coordinate frames, timepoints, and exact provenance/evidence
/// artifact IDs may differ and remain separately visible in the returned runtime wrapper. The
/// available row-count, label, and declaration-digest bindings are checked on each input. Because
/// the compatibility result carries no private receipt or declared-input digest, an otherwise
/// same-label/same-row result supplied through the public runtime fields remains caller-asserted.
/// After the semantic mark gate, the function delegates unchanged to [`compare_marked_prepost`].
/// It infers no row correspondence or biological unit and makes no population, equivalence,
/// noninferiority, causal, or biological claim.
///
/// # Errors
///
/// Returns a category-only error before comparison when an available runtime binding disagrees or
/// when mark identity, label, measurement status, endpoint routing, optional probability identity,
/// or threshold semantics differ.
pub fn compare_declared_marked_prepost<'a>(
    pre: &'a DeclaredMarkedAnalysisResult,
    post: &'a DeclaredMarkedAnalysisResult,
) -> Result<DeclaredMarkedPrePostResult<'a>, DeclaredMarkedPrePostError> {
    if !runtime_binding_matches(pre) {
        return Err(DeclaredMarkedPrePostError::InvalidPreRuntimeBinding);
    }
    if !runtime_binding_matches(post) {
        return Err(DeclaredMarkedPrePostError::InvalidPostRuntimeBinding);
    }
    require_compatible_mark_use(&pre.mark_use, &post.mark_use)?;
    Ok(DeclaredMarkedPrePostResult {
        result: compare_marked_prepost(&pre.result, &post.result),
        pre_mark_use: pre.mark_use.clone(),
        post_mark_use: post.mark_use.clone(),
        pre_scalar_identity: pre.scalar_identity.clone(),
        post_scalar_identity: post.scalar_identity.clone(),
        pre_timepoint: &pre.result.timepoint,
        post_timepoint: &post.result.timepoint,
    })
}

fn runtime_binding_matches(output: &DeclaredMarkedAnalysisResult) -> bool {
    output.result.n_cells == output.scalar_identity.row_count()
        && output.result.mark_label == output.mark_use.binary_mark().label()
        && output.scalar_identity.matches_mark_use(&output.mark_use)
}

fn require_compatible_mark_use(
    pre: &DeclaredMarkUse,
    post: &DeclaredMarkUse,
) -> Result<(), DeclaredMarkedPrePostError> {
    let pre_binary = pre.binary_mark();
    let post_binary = post.binary_mark();
    if pre_binary.mark_id() != post_binary.mark_id() {
        return Err(DeclaredMarkedPrePostError::BinaryMarkIdMismatch);
    }
    if pre_binary.label() != post_binary.label() {
        return Err(DeclaredMarkedPrePostError::BinaryMarkLabelMismatch);
    }
    if pre_binary.measurement_status() != post_binary.measurement_status() {
        return Err(DeclaredMarkedPrePostError::MeasurementStatusMismatch);
    }
    if pre.structure_factor_value_kind() != post.structure_factor_value_kind()
        || pre.other_endpoint_value_kind() != post.other_endpoint_value_kind()
    {
        return Err(DeclaredMarkedPrePostError::EndpointRoutingMismatch);
    }

    match (pre.probability_mark(), post.probability_mark()) {
        (None, None) => {}
        (Some(pre_probability), Some(post_probability)) => {
            if pre_probability.mark_id() != post_probability.mark_id() {
                return Err(DeclaredMarkedPrePostError::ProbabilityMarkIdMismatch);
            }
            if pre_probability.measurement_status() != post_probability.measurement_status() {
                return Err(DeclaredMarkedPrePostError::MeasurementStatusMismatch);
            }
        }
        _ => return Err(DeclaredMarkedPrePostError::EndpointRoutingMismatch),
    }

    if !origins_match(pre_binary.origin(), post_binary.origin()) {
        return Err(DeclaredMarkedPrePostError::BinaryOriginMismatch);
    }
    Ok(())
}

fn origins_match(pre: &BinaryMarkOrigin, post: &BinaryMarkOrigin) -> bool {
    match (pre, post) {
        (BinaryMarkOrigin::Independent, BinaryMarkOrigin::Independent) => true,
        (
            BinaryMarkOrigin::Thresholded {
                probability_mark_id: pre_probability,
                comparator: pre_comparator,
                threshold: pre_threshold,
                ..
            },
            BinaryMarkOrigin::Thresholded {
                probability_mark_id: post_probability,
                comparator: post_comparator,
                threshold: post_threshold,
                ..
            },
        ) => {
            pre_probability == post_probability
                && pre_comparator == post_comparator
                && pre_threshold.to_bits() == post_threshold.to_bits()
        }
        _ => false,
    }
}
