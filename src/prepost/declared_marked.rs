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

/// Availability of a descriptive binary marked-row prevalence change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredMarkedPrevalenceStatus {
    /// Both supplied outputs contain at least one row.
    Available,
    /// At least one supplied output contains no rows.
    InsufficientCells,
}

/// Runtime-only binary prevalence change across two declared marked outputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeclaredMarkedPrevalenceChange<'a> {
    /// Whether a signed prevalence change is available.
    pub status: DeclaredMarkedPrevalenceStatus,
    /// Number of rows in the pre output.
    pub pre_n_cells: usize,
    /// Number of binary-marked rows in the pre output.
    pub pre_n_marked: usize,
    /// Exact canonical binary marked-row prevalence in the pre output.
    pub pre_prevalence: f64,
    /// Number of rows in the post output.
    pub post_n_cells: usize,
    /// Number of binary-marked rows in the post output.
    pub post_n_marked: usize,
    /// Exact canonical binary marked-row prevalence in the post output.
    pub post_prevalence: f64,
    /// Post minus pre binary prevalence, absent when either side has no rows.
    pub delta_prevalence: Option<f64>,
    /// Exact pre-analysis declarations, evidence identities, and endpoint routing.
    pub pre_mark_use: &'a DeclaredMarkUse,
    /// Exact post-analysis declarations, evidence identities, and endpoint routing.
    pub post_mark_use: &'a DeclaredMarkUse,
    /// Exact pre-analysis row, slide, frame, and declaration identity.
    pub pre_scalar_identity: &'a DeclaredScalarIdentity,
    /// Exact post-analysis row, slide, frame, and declaration identity.
    pub post_scalar_identity: &'a DeclaredScalarIdentity,
    /// Exact pre-analysis timepoint metadata.
    pub pre_timepoint: &'a str,
    /// Exact post-analysis timepoint metadata.
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
    require_compatible_outputs(pre, post)?;
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

/// Compare binary marked-row prevalence across two semantically matched declared outputs.
///
/// This descriptive computation always uses `n_marked` and `p_hat`, including when probability
/// values were routed to structure-factor spectra. Different rows, CellIds, slides, frames,
/// timepoints, and evidence identities are retained without implying correspondence or selecting a
/// biological unit. The result has no serializer and makes no paired-cell, patient/specimen,
/// treatment, population, calibration, equivalence, noninferiority, causal, or biological claim.
///
/// # Errors
///
/// Returns the same category-only binding and semantic mismatch errors as
/// [`compare_declared_marked_prepost`].
pub fn compare_declared_marked_prevalence<'a>(
    pre: &'a DeclaredMarkedAnalysisResult,
    post: &'a DeclaredMarkedAnalysisResult,
) -> Result<DeclaredMarkedPrevalenceChange<'a>, DeclaredMarkedPrePostError> {
    require_compatible_outputs(pre, post)?;
    let delta_prevalence = if pre.result.n_cells > 0 && post.result.n_cells > 0 {
        Some(post.result.p_hat - pre.result.p_hat)
    } else {
        None
    };
    Ok(DeclaredMarkedPrevalenceChange {
        status: if delta_prevalence.is_some() {
            DeclaredMarkedPrevalenceStatus::Available
        } else {
            DeclaredMarkedPrevalenceStatus::InsufficientCells
        },
        pre_n_cells: pre.result.n_cells,
        pre_n_marked: pre.result.n_marked,
        pre_prevalence: pre.result.p_hat,
        post_n_cells: post.result.n_cells,
        post_n_marked: post.result.n_marked,
        post_prevalence: post.result.p_hat,
        delta_prevalence,
        pre_mark_use: &pre.mark_use,
        post_mark_use: &post.mark_use,
        pre_scalar_identity: &pre.scalar_identity,
        post_scalar_identity: &post.scalar_identity,
        pre_timepoint: &pre.result.timepoint,
        post_timepoint: &post.result.timepoint,
    })
}

fn require_compatible_outputs(
    pre: &DeclaredMarkedAnalysisResult,
    post: &DeclaredMarkedAnalysisResult,
) -> Result<(), DeclaredMarkedPrePostError> {
    if !runtime_binding_matches(pre) {
        return Err(DeclaredMarkedPrePostError::InvalidPreRuntimeBinding);
    }
    if !runtime_binding_matches(post) {
        return Err(DeclaredMarkedPrePostError::InvalidPostRuntimeBinding);
    }
    require_compatible_mark_use(&pre.mark_use, &post.mark_use)?;
    Ok(())
}

fn runtime_binding_matches(output: &DeclaredMarkedAnalysisResult) -> bool {
    let expected_prevalence = if output.result.n_cells == 0 {
        0.0
    } else {
        output.result.n_marked as f64 / output.result.n_cells as f64
    };
    output.result.n_cells == output.scalar_identity.row_count()
        && output.result.n_marked <= output.result.n_cells
        && output.result.p_hat.is_finite()
        && output.result.p_hat.to_bits() == expected_prevalence.to_bits()
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
