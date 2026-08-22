use crate::{
    api::finite_option,
    common::seeds::{derive_seed, SeedEndpoint},
    config::{AnalysisConfig, ComponentMode},
    data::{validate::validation_flags, Pattern},
    errors::Result,
    geom::{
        components::ComponentSummary,
        length_scales::{bounding_box_diagonal_um, maximum_interpretable_scale_for_points_um},
        spatial_index::mean_nearest_neighbor_distance,
    },
    output::{
        AnalysisSection, ComponentAnalysisSummary, ComponentModeSelection, ResolvedComponentMode,
    },
    spectra::structure_factor::{
        permutation_whitened_spectrum, permutation_whitened_value_spectrum,
        SpectrumPermutationOptions,
    },
};

#[derive(Clone, Debug)]
pub(super) struct ComponentAnalysisPlan {
    pub(super) selection: ComponentModeSelection,
}

impl ComponentAnalysisPlan {
    pub(super) fn includes_pooled(&self) -> bool {
        self.selection.selected != ResolvedComponentMode::Separate
    }

    fn includes_components(&self) -> bool {
        self.selection.selected != ResolvedComponentMode::Pooled
    }
}

pub(super) fn component_analysis_plan(
    config: &AnalysisConfig,
    pattern: &Pattern,
) -> ComponentAnalysisPlan {
    let requested = config.analysis.analyze_components;
    let (selected, reason) = match requested {
        ComponentMode::Pooled => (
            ResolvedComponentMode::Pooled,
            "pooled component mode was explicitly requested".to_owned(),
        ),
        ComponentMode::Separate => (
            ResolvedComponentMode::Separate,
            "separate component mode was explicitly requested; pooled endpoints are not applicable"
                .to_owned(),
        ),
        ComponentMode::Both => (
            ResolvedComponentMode::Both,
            "both pooled and separate component modes were explicitly requested".to_owned(),
        ),
        ComponentMode::Auto => match pattern.component_id.as_deref() {
            None => (
                ResolvedComponentMode::Pooled,
                "auto selected pooled because component IDs are unavailable".to_owned(),
            ),
            Some(component_id) if component_id.len() != pattern.len() => (
                ResolvedComponentMode::Pooled,
                format!(
                    "auto selected pooled because component ID length {} does not match cell count {}",
                    component_id.len(),
                    pattern.len()
                ),
            ),
            Some(component_id) => {
                let summary = ComponentSummary::from_component_ids(component_id);
                if summary.component_count > 1 && summary.largest_fraction < 0.80 {
                    (
                        ResolvedComponentMode::Both,
                        format!(
                            "auto selected both because {} components were present and the largest contained {:.3} of cells (< 0.800)",
                            summary.component_count, summary.largest_fraction
                        ),
                    )
                } else {
                    (
                        ResolvedComponentMode::Pooled,
                        format!(
                            "auto selected pooled because {} component(s) were present and the largest contained {:.3} of cells (threshold 0.800)",
                            summary.component_count, summary.largest_fraction
                        ),
                    )
                }
            }
        },
    };

    ComponentAnalysisPlan {
        selection: ComponentModeSelection {
            requested,
            selected,
            reason,
        },
    }
}

pub(super) fn component_results_for(
    config: &AnalysisConfig,
    pattern: &Pattern,
    plan: &ComponentAnalysisPlan,
) -> Result<AnalysisSection<Vec<ComponentAnalysisSummary>>> {
    if !plan.includes_components() {
        return Ok(AnalysisSection::NotApplicable);
    }
    let Some(component_id) = pattern.component_id.as_deref() else {
        return Ok(AnalysisSection::InsufficientData {
            reason: "separate component analysis requires component IDs".into(),
        });
    };
    if component_id.len() != pattern.len() {
        return Ok(AnalysisSection::InsufficientData {
            reason: format!(
                "component ID length {} does not match cell count {}",
                component_id.len(),
                pattern.len()
            ),
        });
    }
    if component_id.is_empty() {
        return Ok(AnalysisSection::InsufficientData {
            reason: "separate component analysis requires at least one cell".into(),
        });
    }

    let mut component_ids = component_id.to_vec();
    component_ids.sort_unstable();
    component_ids.dedup();

    Ok(AnalysisSection::available(
        component_ids
            .into_iter()
            .map(|id| component_summary_for(config, pattern, component_id, id))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect(),
    ))
}

pub(super) fn component_summary_for(
    config: &AnalysisConfig,
    pattern: &Pattern,
    component_ids: &[u32],
    target_component_id: u32,
) -> Result<Option<ComponentAnalysisSummary>> {
    let indices = component_ids
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, id)| (id == target_component_id).then_some(index))
        .collect::<Vec<_>>();
    if indices.is_empty() {
        return Ok(None);
    }

    let x = indices
        .iter()
        .map(|index| pattern.x_um[*index])
        .collect::<Vec<_>>();
    let y = indices
        .iter()
        .map(|index| pattern.y_um[*index])
        .collect::<Vec<_>>();
    let marks = indices
        .iter()
        .map(|index| pattern.mark[*index])
        .collect::<Vec<_>>();
    let mut component = Pattern::from_arrays(x, y, marks, pattern.meta.clone())?;
    component.window = component_window(pattern, &component);
    if let Some(values) = pattern.mark_prob.as_deref() {
        component.mark_prob = Some(
            indices
                .iter()
                .map(|index| values[*index])
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
    }

    let status_flags = validation_flags(&component, config);
    let options = SpectrumPermutationOptions {
        n_shells: config.spectrum.k_shells,
        low_k_modes: config.spectrum.low_k_shells,
        n_permutations: config.permutation.b,
        seed: derive_seed(
            config.permutation.seed,
            SeedEndpoint::SpectrumComponent,
            target_component_id as usize,
        ),
        family_wise_alpha: config.inference.family_wise_alpha,
        max_scale_um: maximum_interpretable_scale_for_points_um(
            component.window.analysis_effective_length_um,
            &component.x_um,
            &component.y_um,
            config.validation.largest_interpretable_scale_fraction,
        )
        .unwrap_or(0.0),
        k_shell_min: config.validation.k_shell_min,
        k_chunk_modes: config.performance.k_chunk_modes,
    };
    let spectrum = if config.analysis.use_probabilistic_marks {
        if let Some(values) = component.mark_prob.as_deref() {
            let values = values.iter().copied().map(f64::from).collect::<Vec<_>>();
            permutation_whitened_value_spectrum(&component, &values, options)?
        } else {
            None
        }
    } else {
        permutation_whitened_spectrum(&component, options)?
    };

    Ok(Some(ComponentAnalysisSummary {
        component_id: target_component_id,
        n_cells: component.len(),
        n_marked: component.n_marked(),
        p_hat: component.p_hat(),
        status_flags,
        primary_endpoint_value: spectrum.as_ref().map_or_else(
            || crate::output::AnalysisSection::InsufficientData {
                reason: "component spectrum could not be estimated".into(),
            },
            |spectrum| crate::output::AnalysisSection::available(spectrum.low_k_excess),
        ),
        p_global: spectrum
            .as_ref()
            .and_then(|spectrum| finite_option(spectrum.p_global)),
        k_min: spectrum
            .as_ref()
            .and_then(|spectrum| spectrum.k_values.first().copied())
            .and_then(finite_option),
        k_max: spectrum
            .as_ref()
            .and_then(|spectrum| spectrum.k_values.last().copied())
            .and_then(finite_option),
        n_k_modes: spectrum
            .as_ref()
            .map(|spectrum| spectrum.n_modes)
            .unwrap_or(0),
        xi_um: spectrum
            .as_ref()
            .and_then(|spectrum| spectrum.xi_um)
            .and_then(finite_option),
        alpha: if config.spectrum.fit_low_k_alpha {
            spectrum
                .as_ref()
                .and_then(|spectrum| spectrum.alpha)
                .and_then(finite_option)
        } else {
            None
        },
    }))
}

pub(super) fn component_window(parent: &Pattern, component: &Pattern) -> crate::data::TumorWindow {
    let area_fraction = if parent.is_empty() {
        0.0
    } else {
        component.len() as f64 / parent.len() as f64
    };
    let area_um2 = parent.window.area_um2 * area_fraction;
    crate::data::TumorWindow {
        area_um2,
        analysis_effective_length_um: component_bounding_box_diagonal_um(component).unwrap_or(0.0),
        d_nn_mean_um: mean_nearest_neighbor_distance(&component.x_um, &component.y_um)
            .unwrap_or(parent.window.d_nn_mean_um),
        valid_mask_fraction: parent.window.valid_mask_fraction,
    }
}

pub(super) fn component_bounding_box_diagonal_um(pattern: &Pattern) -> Option<f64> {
    bounding_box_diagonal_um(&pattern.x_um, &pattern.y_um)
}
