use crate::{
    api::finite_option,
    common::{
        seeds::{derive_seed, SeedEndpoint},
        stats::min_max_ignoring_nonfinite,
    },
    config::{AnalysisConfig, ComponentMode},
    data::{validate::validation_flags, Pattern},
    errors::Result,
    geom::{components::ComponentSummary, spatial_index::mean_nearest_neighbor_distance},
    output::ComponentAnalysisSummary,
    spectra::structure_factor::{
        permutation_whitened_spectrum, permutation_whitened_value_spectrum,
        SpectrumPermutationOptions,
    },
};

pub(super) fn component_results_for(
    config: &AnalysisConfig,
    pattern: &Pattern,
) -> Result<Vec<ComponentAnalysisSummary>> {
    let Some(component_id) = pattern.component_id.as_deref() else {
        return Ok(Vec::new());
    };
    if component_id.len() != pattern.len() || !should_emit_component_results(config, component_id) {
        return Ok(Vec::new());
    }

    let mut component_ids = component_id.to_vec();
    component_ids.sort_unstable();
    component_ids.dedup();

    Ok(component_ids
        .into_iter()
        .map(|id| component_summary_for(config, pattern, component_id, id))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

pub(super) fn should_emit_component_results(config: &AnalysisConfig, component_id: &[u32]) -> bool {
    match config.analysis.analyze_components {
        ComponentMode::Pooled => false,
        ComponentMode::Separate | ComponentMode::Both => true,
        ComponentMode::Auto => {
            let summary = ComponentSummary::from_component_ids(component_id);
            summary.component_count > 1 && summary.largest_fraction < 0.80
        }
    }
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
        max_scale_um: config.validation.largest_interpretable_scale_fraction
            * component.window.l_eff_um,
        k_shell_min: config.validation.k_shell_min,
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
        l_eff_um: component_l_eff_um(component).unwrap_or(parent.window.l_eff_um),
        d_nn_mean_um: mean_nearest_neighbor_distance(&component.x_um, &component.y_um)
            .unwrap_or(parent.window.d_nn_mean_um),
        valid_mask_fraction: parent.window.valid_mask_fraction,
    }
}

pub(super) fn component_l_eff_um(pattern: &Pattern) -> Option<f64> {
    if pattern.is_empty() {
        return None;
    }
    let (min_x, max_x) = min_max_ignoring_nonfinite(&pattern.x_um)?;
    let (min_y, max_y) = min_max_ignoring_nonfinite(&pattern.y_um)?;
    Some((max_x - min_x).hypot(max_y - min_y).max(1.0))
}
