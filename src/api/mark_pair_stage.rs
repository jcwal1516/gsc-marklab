use crate::{
    api::{finite_option, qc_pipeline::permutation_labels},
    common::seeds::SeedEndpoint,
    config::AnalysisConfig,
    data::Pattern,
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    output::{AnalysisSection, FunctionalSummary, MarkPairCovariancePoint},
    permutation::envelopes::GlobalEnvelope,
    spectra::mark_pair_covariance::MarkPairCovariancePlan,
};

pub(super) fn mark_pair_covariance_with_envelope(
    config: &AnalysisConfig,
    pattern: &Pattern,
    spatial_index: &SpatialIndex2D,
    geometry_budget_bytes: usize,
) -> Result<(
    Vec<MarkPairCovariancePoint>,
    AnalysisSection<FunctionalSummary>,
    usize,
)> {
    let bin_width_um = pattern.window.d_nn_mean_um.max(1.0);
    let max_r_um =
        (pattern.window.l_eff_um * config.validation.largest_interpretable_scale_fraction).max(1.0);
    let index_storage_bytes = spatial_index.estimated_storage_bytes();
    let plan_budget_bytes = geometry_budget_bytes.saturating_sub(index_storage_bytes);
    let Some(plan) = MarkPairCovariancePlan::new_with_index(
        pattern,
        spatial_index,
        bin_width_um,
        max_r_um,
        plan_budget_bytes,
    )?
    else {
        return Ok((
            Vec::new(),
            AnalysisSection::InsufficientData {
                reason: "mark-pair covariance geometry could not be planned".into(),
            },
            index_storage_bytes,
        ));
    };
    let geometry_storage_bytes = index_storage_bytes.saturating_add(plan.estimated_storage_bytes());
    let Some(observed_bins) = plan.evaluate(&pattern.mark) else {
        return Ok((
            Vec::new(),
            AnalysisSection::InsufficientData {
                reason: "mark-pair covariance could not be estimated".into(),
            },
            geometry_storage_bytes,
        ));
    };

    let observed_values = observed_bins
        .iter()
        .map(|bin| bin.value.unwrap_or(0.0))
        .collect::<Vec<_>>();
    let inference_eligible = observed_bins
        .iter()
        .map(|bin| bin.value.is_some() && bin.r_max_um <= max_r_um)
        .collect::<Vec<_>>();
    if observed_bins
        .iter()
        .filter_map(|bin| bin.value)
        .any(|value| !value.is_finite())
    {
        return Err(MarklabError::Compute(
            "observed mark-pair-covariance curve contains a non-finite value".into(),
        ));
    }
    let permutation_curves =
        mark_pair_covariance_permutation_curves(config, pattern, &plan, observed_values.len())?;
    let envelope = match permutation_curves {
        Some(permutation_curves) if inference_eligible.iter().any(|eligible| *eligible) => {
            Some(GlobalEnvelope::from_curves_with_eligibility(
                &observed_values,
                &permutation_curves,
                config.inference.family_wise_alpha,
                &inference_eligible,
            )?)
        }
        None | Some(_) => None,
    };
    let summary = envelope.as_ref().map_or_else(
        || AnalysisSection::InsufficientData {
            reason: "at least one required mark-pair-covariance null curve was undefined".into(),
        },
        |envelope| {
            AnalysisSection::available(FunctionalSummary {
                p_global: finite_option(envelope.p_global),
                erl_depth: Some(envelope.erl_depth),
                n_permutations: envelope.n_permutations,
            })
        },
    );

    let points = observed_bins
        .into_iter()
        .enumerate()
        .map(|(index, bin)| {
            let envelope_bounds = bin.value.and(envelope.as_ref()).and_then(|envelope| {
                let lower = envelope.lower.get(index).copied().and_then(finite_option)?;
                let upper = envelope.upper.get(index).copied().and_then(finite_option)?;
                Some((lower, upper))
            });
            if !bin.r_min_um.is_finite() || !bin.r_max_um.is_finite() {
                return Err(MarklabError::Compute(format!(
                    "mark-pair-covariance bin {index} has non-finite bounds"
                )));
            }
            Ok(MarkPairCovariancePoint {
                r_min_um: bin.r_min_um,
                r_max_um: bin.r_max_um,
                covariance: bin.value,
                inference_eligible: inference_eligible[index],
                lower_global_envelope: envelope_bounds.map(|bounds| bounds.0),
                upper_global_envelope: envelope_bounds.map(|bounds| bounds.1),
                pair_count: bin.count,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok((points, summary, geometry_storage_bytes))
}

fn mark_pair_covariance_permutation_curves(
    config: &AnalysisConfig,
    pattern: &Pattern,
    plan: &MarkPairCovariancePlan,
    expected_len: usize,
) -> Result<Option<Vec<Vec<f64>>>> {
    if config.permutation.b == 0 || pattern.n_marked() == 0 || pattern.n_unmarked() == 0 {
        return Ok(None);
    }

    let mut curves = Vec::with_capacity(config.permutation.b);
    for perm_index in 0..config.permutation.b {
        let labels = permutation_labels(
            config,
            pattern,
            perm_index,
            SeedEndpoint::MarkPairCovariance,
        )?;
        let Some(bins) = plan.evaluate(&labels) else {
            return Ok(None);
        };
        if bins.len() != expected_len
            || bins
                .iter()
                .filter_map(|bin| bin.value)
                .any(|value| !value.is_finite())
        {
            return Ok(None);
        }
        curves.push(
            bins.into_iter()
                .map(|bin| bin.value.unwrap_or(0.0))
                .collect(),
        );
    }
    Ok(Some(curves))
}
