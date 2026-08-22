use crate::output::{AnalysisSection, DiagnosticsResult, MarkedPatternResult, MultimodalResult};

pub fn render_analysis_report(result: &MarkedPatternResult) -> String {
    let curve_comparison_framing = curve_comparison_framing(result);
    let diagnostics = render_diagnostics(&result.diagnostics);
    let p_global = result
        .primary_endpoint
        .p_value
        .value()
        .copied()
        .map(|value| format!("{value:.4}"))
        .unwrap_or_else(|| "not available".into());
    let spectrum = result.spectrum.value();
    let anisotropy = result.anisotropy.value();
    let multiscale_residual = result.multiscale_residual.value();
    let xi = spectrum
        .and_then(|value| value.xi_um)
        .map(|value| format!("{value:.3} um"))
        .unwrap_or_else(|| "not available".into());
    let anisotropy_p = anisotropy
        .and_then(|value| value.p_value)
        .map(|value| format!("{value:.4}"))
        .unwrap_or_else(|| "not available".into());
    let theta = anisotropy
        .and_then(|value| value.theta_deg)
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "not available".into());
    let low_k = spectrum
        .map(|value| format!("{:.4}", value.low_k_excess))
        .unwrap_or_else(|| "not available".into());
    let max_scale = spectrum
        .map(|value| format!("{:.3} um", value.max_interpretable_scale_um))
        .unwrap_or_else(|| "not available".into());
    let n_k_modes = spectrum
        .map(|value| value.n_k_modes.to_string())
        .unwrap_or_else(|| "not available".into());
    let n_shells = spectrum
        .map(|value| value.n_shells.to_string())
        .unwrap_or_else(|| "not available".into());
    let anisotropy_index = anisotropy
        .map(|value| format!("{:.4}", value.index))
        .unwrap_or_else(|| "not available".into());
    let block_mean = multiscale_residual
        .map(|value| format!("{:.4}", value.block_mean_variance_fraction))
        .unwrap_or_else(|| "not available".into());
    let territories = multiscale_residual
        .map(|value| value.territory_count.to_string())
        .unwrap_or_else(|| "not available".into());

    format!(
        "# Marklab Analysis Report\n\n\
Case: {case_id}\n\n\
Timepoint: {timepoint}\n\n\
Protein: {protein}\n\n\
Mark label: {mark_label}\n\n\
Status: {status}\n\n\
Cells: {n_cells} total, {n_marked} {mark_label}, p_hat = {p_hat:.4}\n\n\
Window: L_eff = {l_eff:.3} um, mean nearest-neighbor distance = {dnn:.3} um\n\n\
Primary endpoint: low-k excess = {low_k}; scalar p-value = {p_global}; null = {null_model}\n\n\
Spectrum: xi = {xi}; raw k modes = {n_k_modes}; radial shells = {n_shells}; maximum interpretable scale = {max_scale}\n\n\
Anisotropy: index = {anisotropy}; theta_deg = {theta}; p-value = {anisotropy_p}\n\n\
Multiscale residual: block-mean variance fraction = {block_mean}; residual-neighborhood territories = {territories}\n\n\
Interpretation: {interpretation}\n\n\
{curve_comparison_framing}\
{diagnostics}\
Scientific framing: This report quantifies section-level organization of the configured mark field relative to fixed-position random labeling. Domain-specific biological interpretation requires a separate, explicitly scoped policy.\n",
        case_id = result.case_id,
        timepoint = result.timepoint,
        protein = result.protein,
        mark_label = result.mark_label,
        status = result.status,
        n_cells = result.n_cells,
        n_marked = result.n_marked,
        p_hat = result.p_hat,
        l_eff = result.window.l_eff_um,
        dnn = result.window.d_nn_mean_um,
        low_k = low_k,
        p_global = p_global,
        null_model = result.primary_endpoint.null,
        xi = xi,
        n_k_modes = n_k_modes,
        n_shells = n_shells,
        max_scale = max_scale,
        anisotropy = anisotropy_index,
        theta = theta,
        anisotropy_p = anisotropy_p,
        block_mean = block_mean,
        territories = territories,
        interpretation = result.interpretation.text,
        curve_comparison_framing = curve_comparison_framing,
        diagnostics = diagnostics,
    )
}

pub fn render_multimodal_report(result: &MultimodalResult) -> String {
    let mut report = format!(
        "# Marklab Multimodal Analysis Report\n\n\
Case: {case_id}\n\n\
Status: {status}\n\n\
Interpretation: {interpretation}\n\n\
Scientific framing: This report summarizes serial-section cells placed in a shared coordinate frame. H&E and IHC records are not same-cell matches, and the results should be interpreted as section-level spatial associations rather than molecular confirmation or cell tracking.\n",
        case_id = result.case_id,
        status = result.status,
        interpretation = result.interpretation.text,
    );

    report.push_str(
        "\nMultimodal associations are serial-section neighborhood associations, not same-cell claims. Associations below the registration uncertainty scale are diagnostic only.\n",
    );
    if result
        .territory_comparisons
        .value()
        .is_some_and(|value| !value.is_empty())
    {
        report.push_str("Curve comparisons: pooled-bin permutation diagnostics describe difference, while descriptive margin assessments only report whether the chosen curve distance is within a configured margin. A nonsignificant difference diagnostic is not interpreted as sameness.\n\n");
    }
    report.push_str(&render_diagnostics(&result.diagnostics));

    if let Some(registration) = result.registration.value() {
        report.push_str(&format!(
            "\n## Multimodal Registration\n\n\
Transform: {transform_type}\n\n\
Landmarks: {landmark_count}\n\n\
RMSE: {rmse_um:.3} um\n\n\
P95 residual: {p95_residual_um:.3} um\n\n\
Usable minimum distance: {usable_min_distance_um:.3} um\n",
            transform_type = registration.transform_type,
            landmark_count = registration.landmark_count,
            rmse_um = registration.rmse_um,
            p95_residual_um = registration.p95_residual_um,
            usable_min_distance_um = registration.usable_min_distance_um,
        ));
    }

    if let Some(summary) = result.fused_cell_summary.value() {
        report.push_str(&format!(
            "\n## Fused Cells\n\n\
H&E cells: {n_he_cells}\n\n\
IHC cells: {n_ihc_cells}\n\n\
Fused cells: {n_fused_cells}\n",
            n_he_cells = summary.n_he_cells,
            n_ihc_cells = summary.n_ihc_cells,
            n_fused_cells = summary.n_fused_cells,
        ));
    }

    if let Some(enrichment) = result
        .neighborhood_enrichment
        .value()
        .filter(|value| !value.is_empty())
    {
        report.push_str("\n## Neighborhood Enrichment\n\n");
        for row in enrichment {
            let enrichment_ratio = row.enrichment_ratio.map_or_else(
                || {
                    format!(
                        "undefined ({})",
                        row.enrichment_ratio_unavailable_reason
                            .map_or("reason_unavailable", |reason| reason.as_str())
                    )
                },
                |value| format!("{value:.3}"),
            );
            report.push_str(&format!(
                "- {label_a} / {label_b}: observed_edges = {observed_edges}, expected_edges = {expected_edges:.3}, enrichment_ratio = {enrichment_ratio}\n",
                label_a = row.label_a,
                label_b = row.label_b,
                observed_edges = row.observed_edges,
                expected_edges = row.expected_edges,
            ));
        }
    }

    report
}

fn render_diagnostics(section: &AnalysisSection<DiagnosticsResult>) -> String {
    let Some(diagnostics) = section.value() else {
        return String::new();
    };

    let mut text = String::from("Optional diagnostics: exploratory summaries only; primary statistical endpoints remain unchanged.\n\n");
    if let Some(beta_posterior_groups) = &diagnostics.beta_posterior_groups {
        text.push_str(&format!(
            "Beta posterior group summary: {diagnostic_name}; posterior mean = {posterior_mean:.4}; 95% interval = [{lower:.4}, {upper:.4}]; group posterior mean range = {range:.4}.\n\n",
            diagnostic_name = beta_posterior_groups.diagnostic_name,
            posterior_mean = beta_posterior_groups.posterior_mean,
            lower = beta_posterior_groups.credible_interval_95[0],
            upper = beta_posterior_groups.credible_interval_95[1],
            range = beta_posterior_groups.group_posterior_mean_range,
        ));
    }
    if let Some(graph_smoothing) = &diagnostics.graph_smoothing {
        text.push_str(&format!(
            "Graph-smoothing summary: {diagnostic_name}; nodes = {nodes}; edges = {edges}; mean degree = {mean_degree:.3}; below-registration-resolution edge fraction = {below:.4}.\n\n",
            diagnostic_name = graph_smoothing.diagnostic_name,
            nodes = graph_smoothing.n_nodes,
            edges = graph_smoothing.n_edges,
            mean_degree = graph_smoothing.mean_degree,
            below = graph_smoothing.below_registration_resolution_edge_fraction,
        ));
    }
    text
}

fn curve_comparison_framing(result: &MarkedPatternResult) -> String {
    if result.prepost_curve_comparisons.is_empty()
        && result
            .territory_comparisons
            .value()
            .is_none_or(Vec::is_empty)
    {
        return String::new();
    }

    "Curve comparisons: pooled-bin permutation diagnostics describe difference, while descriptive margin assessments only report whether the chosen curve distance is within a configured margin. A nonsignificant difference diagnostic is not interpreted as sameness.\n\n".into()
}
