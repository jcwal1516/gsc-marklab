//! Concrete multiplex panel → slide summaries → independent-patient inference application.

mod admission;
mod model;
mod nodes;
mod publication;
mod reduction;
mod workflow;

pub use workflow::{execute_multiplex_study, MultiplexStudyRun, MultiplexStudyTarget};

pub use model::MultiplexStudyResult;
pub use publication::publish_multiplex_study;

use crate::{
    summarize_assay_spatial, AssaySpatialInput, AssaySpatialLimits, AssaySpatialOutcome,
    AssaySpatialUnavailable, MarklabError, Result,
};
use admission::{PreparedSlide, PreparedStudy};
use model::{ChannelResult, SlideResult};

/// Analyze a complete version-one multiplex study using canonical Moran/Geary and patient Max-T.
///
/// The strict JSON recipe freezes physical coordinates, assay metadata, missingness, endpoints,
/// patient identities/reduction, exchangeability and bounded work. Unknown fields are rejected.
/// All selected slide endpoints must be available for cohort inference; failures never silently
/// remove slides or patients. This direct service performs no filesystem I/O. Durable execution
/// consumes the same admission, slide and reduction owners. Errors identify invalid inputs or
/// exhausted resources; scientific unavailability is retained in the typed result.
pub fn analyze_multiplex_study(recipe_json: &[u8]) -> Result<MultiplexStudyResult> {
    let study = PreparedStudy::from_json(recipe_json)?;
    let mut slides = Vec::with_capacity(study.slides.len());
    let mut total_edge_work = 0;
    for slide in &study.slides {
        let result = analyze_slide(
            &study,
            slide,
            study.recipe.limits.maximum_edge_evaluations - total_edge_work,
        )?;
        workflow::count_edge_work(
            &mut total_edge_work,
            &result,
            study.recipe.limits.maximum_edge_evaluations,
        )?;
        slides.push(result);
    }
    reduction::reduce(&study, slides)
}

fn analyze_slide(
    study: &PreparedStudy,
    slide: &PreparedSlide,
    remaining_edge_work: usize,
) -> Result<SlideResult> {
    let design = &study.recipe.design;
    let limits = study.recipe.limits;
    let summaries = summarize_assay_spatial(
        AssaySpatialInput {
            table: &slide.table,
            x_um: &slide.x,
            y_um: &slide.y,
            coordinate_frame_id: &slide.frame,
        },
        &slide.window,
        &study.selection,
        design.radius_um,
        study.weight_policy,
        AssaySpatialLimits {
            maximum_points: limits.maximum_rows_per_slide,
            maximum_directed_edges: limits.maximum_directed_edges,
            maximum_channels: study.selection.len(),
            maximum_edge_evaluations: remaining_edge_work.max(1),
        },
    )
    .map_err(|error| invalid(format!("slide {}: {error}", slide.source.slide_id)))?;
    let channels = summaries
        .into_iter()
        .map(|summary| {
            let (status, reason, moran_i, geary_c) = match summary.outcome {
                AssaySpatialOutcome::Available { moran_i, geary_c } => {
                    ("available", None, Some(moran_i), Some(geary_c))
                }
                AssaySpatialOutcome::Unavailable { reason } => (
                    "unavailable",
                    Some(
                        match reason {
                            AssaySpatialUnavailable::InsufficientObservedPoints => {
                                "insufficient_observed_points"
                            }
                            AssaySpatialUnavailable::IsolatedObservedPoint => {
                                "isolated_observed_point"
                            }
                            AssaySpatialUnavailable::ZeroVariance => "zero_variance",
                            AssaySpatialUnavailable::NumericalFailure => "numerical_failure",
                        }
                        .to_owned(),
                    ),
                    None,
                    None,
                ),
            };
            ChannelResult {
                channel: summary.mark_id.as_str().into(),
                status: status.into(),
                reason,
                observed_rows: summary.observed_rows,
                missing_rows: summary.missing_rows,
                directed_edges: summary.directed_edges,
                weights_digest: summary.weights_digest.map(|digest| digest.to_string()),
                moran_i,
                geary_c,
            }
        })
        .collect();
    Ok(SlideResult {
        slide_id: slide.source.slide_id.clone(),
        patient_id: slide.source.patient_id.clone(),
        group: slide.source.group.clone(),
        coordinate_frame_id: slide.frame.as_str().into(),
        source_sha256: slide.source_artifact.digest().to_string(),
        panel_identity: slide.panel_artifact.digest().to_string(),
        window_identity: slide.window.descriptor().logical_digest.to_string(),
        cell_count: slide.table.cell_ids().len(),
        channels,
    })
}

fn invalid(reason: impl Into<String>) -> MarklabError {
    MarklabError::Validation(format!("multiplex study: {}", reason.into()))
}
