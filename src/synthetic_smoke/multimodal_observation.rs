use crate::{
    errors::{MarklabError, Result},
    multimodal::{MultimodalAnalysisRun, MultimodalEngine},
    output::{
        CurveComparisonMethod, EnrichmentStatisticUnavailableReason, MultimodalResult,
        NeighborhoodEnrichmentResult,
    },
    prepost::compare_multimodal_prepost_with_margin,
};

use super::generators::{self, multimodal_replicate_scenario};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ObservedMultimodalOutcome {
    pub(super) criterion_met: bool,
    pub(super) detected: bool,
    pub(super) false_positive: bool,
    pub(super) below_registration_resolution: bool,
    pub(super) within_margin: bool,
}

pub(super) fn run_multimodal_replicate(
    generator: &str,
    seed: u64,
    generator_index: u64,
    replicate: usize,
) -> Result<ObservedMultimodalOutcome> {
    let scenario = multimodal_replicate_scenario(generator, seed, generator_index, replicate)?;
    let engine = MultimodalEngine::new(scenario.config.clone())?;
    let pre = match generator {
        "registration_residual_above_threshold" => {
            return expected_engine_error_outcome(
                engine.analyze_run(&scenario.pre),
                "exceeds configured max_rmse_um",
            );
        }
        "too_few_landmarks" => {
            return expected_engine_error_outcome(
                engine.analyze_run(&scenario.pre),
                "registration requires at least",
            );
        }
        "degenerate_landmarks" => {
            return expected_engine_error_outcome(
                engine.analyze_run(&scenario.pre),
                "must span nonzero distance",
            );
        }
        _ => engine.analyze_run(&scenario.pre)?,
    };

    match generator {
        "random_labels_no_association"
        | "immune_independent_mmr_territory"
        | "registration_jitter_no_association" => {
            let detected = immune_enrichment_detected(&pre.result)?;
            Ok(ObservedMultimodalOutcome {
                criterion_met: !detected,
                detected,
                false_positive: detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "two_unrelated_mmr_territories" => {
            let false_positive = territory_count(&pre.result)? == 1;
            Ok(ObservedMultimodalOutcome {
                criterion_met: !false_positive,
                false_positive,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "two_related_mmr_territories" => {
            let detected = territory_count(&pre.result)? == 1;
            Ok(ObservedMultimodalOutcome {
                criterion_met: detected,
                detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "immune_associated_mmr_territory" => {
            let detected = immune_enrichment_detected(&pre.result)?;
            Ok(ObservedMultimodalOutcome {
                criterion_met: detected,
                detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "cross_interaction_enrichment" => {
            let detected = cross_interaction_detected(&pre.result)?;
            Ok(ObservedMultimodalOutcome {
                criterion_met: detected,
                detected,
                ..ObservedMultimodalOutcome::default()
            })
        }
        "registration_jitter" => {
            let detected = immune_enrichment_detected(&pre.result)?;
            let below_registration_resolution = pre
                .graph
                .edges
                .iter()
                .any(|edge| edge.below_registration_resolution);
            let false_positive = detected && !below_registration_resolution;
            Ok(ObservedMultimodalOutcome {
                criterion_met: below_registration_resolution && !false_positive,
                detected,
                false_positive,
                below_registration_resolution,
                within_margin: false,
            })
        }
        "prepost_within_margin_spatial_pattern" | "prepost_changed_spatial_pattern" => {
            prepost_outcome(generator, &scenario, &engine, &pre)
        }
        "empty_he_cells" => {
            let summary = fused_cell_summary(&pre.result)?;
            Ok(criterion_outcome(
                summary.n_he_cells == 0 && summary.n_ihc_cells > 0,
            ))
        }
        "empty_ihc_cells" => {
            let summary = fused_cell_summary(&pre.result)?;
            Ok(criterion_outcome(
                summary.n_ihc_cells == 0 && summary.n_he_cells > 0,
            ))
        }
        "no_abnormal_cells" => Ok(criterion_outcome(territory_count(&pre.result)? == 0)),
        "sparse_graph" => Ok(criterion_outcome(pre.graph.edges.is_empty())),
        "zero_expected_edge_count" => {
            let row = lymphocyte_enrichment(&pre.result)?;
            Ok(criterion_outcome(
                row.expected_edges == 0.0
                    && row.enrichment_ratio.is_none()
                    && row.enrichment_ratio_unavailable_reason
                        == Some(EnrichmentStatisticUnavailableReason::ZeroExpectedEdges),
            ))
        }
        "multiple_cell_classes" => Ok(criterion_outcome(
            pre.result
                .neighborhood_enrichment
                .value()
                .is_some_and(|rows| rows.len() == 3),
        )),
        "multiple_null_models" => Ok(criterion_outcome(pre.null_model_sensitivity.len() == 4)),
        "rigid_rotation" => Ok(criterion_outcome(
            approximately(pre.transform.m00, 0.0)
                && approximately(pre.transform.m01, -1.0)
                && approximately(pre.transform.m10, 1.0)
                && approximately(pre.transform.m11, 0.0)
                && approximately(pre.transform.m02, 10.0)
                && approximately(pre.transform.m12, -5.0),
        )),
        "affine_deformation" => Ok(criterion_outcome(
            approximately(pre.transform.m00, 1.1)
                && approximately(pre.transform.m01, 0.2)
                && approximately(pre.transform.m10, -0.1)
                && approximately(pre.transform.m11, 0.9)
                && approximately(pre.transform.m02, 3.0)
                && approximately(pre.transform.m12, -4.0),
        )),
        _ => Err(MarklabError::Validation(format!(
            "unknown multimodal synthetic generator {generator}"
        ))),
    }
}

fn expected_engine_error_outcome(
    result: Result<MultimodalAnalysisRun>,
    expected_message: &str,
) -> Result<ObservedMultimodalOutcome> {
    match result {
        Err(error) if error.to_string().contains(expected_message) => Ok(criterion_outcome(true)),
        Err(error) => Err(MarklabError::Validation(format!(
            "production engine returned an unexpected error: {error}"
        ))),
        Ok(_) => Ok(criterion_outcome(false)),
    }
}

fn criterion_outcome(criterion_met: bool) -> ObservedMultimodalOutcome {
    ObservedMultimodalOutcome {
        criterion_met,
        ..ObservedMultimodalOutcome::default()
    }
}

fn territory_count(result: &MultimodalResult) -> Result<usize> {
    result
        .neighborhood_territories
        .value()
        .map(Vec::len)
        .ok_or_else(|| {
            MarklabError::Validation(
                "production multimodal result did not provide neighborhood territories".into(),
            )
        })
}

fn immune_enrichment_detected(result: &MultimodalResult) -> Result<bool> {
    let row = lymphocyte_enrichment(result)?;
    let p_value = row.q_value.or(row.p_value).ok_or_else(|| {
        MarklabError::Validation(
            "production lymphocyte/mmr_abnormal enrichment was not evaluable".into(),
        )
    })?;
    Ok(p_value <= 0.05)
}

fn lymphocyte_enrichment(result: &MultimodalResult) -> Result<&NeighborhoodEnrichmentResult> {
    let enrichment = result.neighborhood_enrichment.value().ok_or_else(|| {
        MarklabError::Validation(
            "production multimodal result did not provide neighborhood enrichment".into(),
        )
    })?;
    let row = enrichment
        .iter()
        .find(|row| row.label_a == "lymphocyte" && row.label_b == "mmr_abnormal")
        .ok_or_else(|| {
            MarklabError::Validation(
                "production multimodal result omitted lymphocyte/mmr_abnormal enrichment".into(),
            )
        })?;
    Ok(row)
}

fn cross_interaction_detected(result: &MultimodalResult) -> Result<bool> {
    result
        .cross_interaction_curves
        .value()
        .and_then(|curves| {
            curves.iter().find(|curve| {
                curve.label_a == "lymphocyte" && curve.label_b == "mmr_abnormal"
                    || curve.label_a == "mmr_abnormal" && curve.label_b == "lymphocyte"
            })
        })
        .and_then(|curve| curve.p_global)
        .map(|p_value| p_value <= 0.05)
        .ok_or_else(|| {
            MarklabError::Validation(
                "production multimodal result omitted evaluable lymphocyte/mmr_abnormal cross interaction"
                    .into(),
            )
        })
}

fn fused_cell_summary(result: &MultimodalResult) -> Result<&crate::output::FusedCellSummary> {
    result.fused_cell_summary.value().ok_or_else(|| {
        MarklabError::Validation(
            "production multimodal result did not provide a fused-cell summary".into(),
        )
    })
}

fn approximately(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1.0e-10
}

fn prepost_outcome(
    generator: &str,
    scenario: &generators::MultimodalScenario,
    engine: &MultimodalEngine,
    pre: &MultimodalAnalysisRun,
) -> Result<ObservedMultimodalOutcome> {
    let post_input = scenario.post.as_ref().ok_or_else(|| {
        MarklabError::Validation("pre/post smoke scenario omitted the post input".into())
    })?;
    let post = engine.analyze_run(post_input)?;
    let comparison = compare_multimodal_prepost_with_margin(
        &pre.result,
        &post.result,
        scenario.config.comparison.margins.cross_interaction,
    );
    let within_margin = comparison
        .curve_comparisons
        .iter()
        .find(|comparison| comparison.method == CurveComparisonMethod::DescriptiveMargin)
        .and_then(|comparison| comparison.within_margin)
        .ok_or_else(|| {
            MarklabError::Validation(
                "production pre/post comparison did not provide a descriptive margin result".into(),
            )
        })?;

    Ok(ObservedMultimodalOutcome {
        criterion_met: if generator == "prepost_within_margin_spatial_pattern" {
            within_margin
        } else {
            !within_margin
        },
        detected: !within_margin,
        false_positive: !within_margin,
        below_registration_resolution: false,
        within_margin,
    })
}
