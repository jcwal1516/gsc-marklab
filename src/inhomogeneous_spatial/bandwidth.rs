use marklab_workflow::ContentDigest;

use crate::{ObservationWindow2D, Pattern};

use super::{
    analysis::{compensated_add, Counters},
    analyze_inhomogeneous_spatial_pattern,
    bandwidth_types::{
        GaussianBandwidthCandidateScore, GaussianBandwidthSelectionConfig,
        GaussianBandwidthSelectionSummary, SelectedInhomogeneousSpatialResult,
    },
    intensity::{build_grid, extrema, fit_event_intensity},
    InhomogeneousSpatialConfig, InhomogeneousSpatialError, InhomogeneousSpatialLimits,
};

/// Select a Gaussian bandwidth by prespecified leave-one-out likelihood, then run K/L once.
///
/// Candidate scoring uses only event intensities. No K/L/g value, envelope, or
/// deviation enters selection. Exact score ties retain the first (smallest)
/// bandwidth because candidates are strictly increasing.
pub fn analyze_selected_inhomogeneous_spatial_pattern(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &GaussianBandwidthSelectionConfig,
) -> Result<SelectedInhomogeneousSpatialResult, InhomogeneousSpatialError> {
    if pattern.len() < 2 {
        return Err(InhomogeneousSpatialError::InsufficientPoints);
    }
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
        || pattern.len() > config.analysis_limits.maximum_points
    {
        return Err(InhomogeneousSpatialError::Dependency(
            "pattern shape, validity, or point limit is invalid".into(),
        ));
    }
    let selection_limits = selection_analysis_limits(config)?;
    let first_config =
        candidate_config(config, config.candidate_bandwidths_um[0], selection_limits)?;
    let grid = build_grid(window, &first_config)?;
    let mut counters = Counters {
        intensity_evaluations: 0,
        pair_visits: 0,
        null_draws: 0,
    };
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(config.candidate_bandwidths_um.len())
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    let mut selected_index = 0_usize;
    let mut selected_score = f64::NEG_INFINITY;
    for (index, bandwidth_um) in config.candidate_bandwidths_um.iter().copied().enumerate() {
        let candidate = candidate_config(config, bandwidth_um, selection_limits)?;
        let starting_evaluations = counters.intensity_evaluations;
        let fitted = fit_event_intensity(pattern, &grid, &candidate, &mut counters)?;
        let (minimum, maximum) = extrema(&fitted.observed_intensities)?;
        let mut log_sum = 0.0;
        let mut correction = 0.0;
        for intensity in fitted.observed_intensities {
            compensated_add(&mut log_sum, &mut correction, intensity.ln());
        }
        let score = (log_sum + correction) / pattern.len() as f64;
        if !score.is_finite() {
            return Err(InhomogeneousSpatialError::Dependency(
                "bandwidth leave-one-out score is non-finite".into(),
            ));
        }
        if score > selected_score {
            selected_index = index;
            selected_score = score;
        }
        candidates.push(GaussianBandwidthCandidateScore {
            bandwidth_um,
            mean_log_leave_one_out_intensity: score,
            minimum_intensity_per_um2: minimum,
            maximum_intensity_per_um2: maximum,
            intensity_evaluations: counters.intensity_evaluations - starting_evaluations,
        });
    }
    let selected_bandwidth_um = config.candidate_bandwidths_um[selected_index];
    let analysis_config = candidate_config(config, selected_bandwidth_um, config.analysis_limits)?;
    let analysis = analyze_inhomogeneous_spatial_pattern(pattern, window, &analysis_config)?;
    let estimated_storage_bytes = selected_retained_bytes(&analysis, candidates.len())?;
    if estimated_storage_bytes > config.analysis_limits.maximum_retained_bytes {
        return Err(InhomogeneousSpatialError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.analysis_limits.maximum_retained_bytes,
        });
    }
    let artifact_digest = selection_digest(pattern, window, config, &candidates, selected_index);
    Ok(SelectedInhomogeneousSpatialResult {
        selection: GaussianBandwidthSelectionSummary {
            method: "leave_one_out_log_likelihood".into(),
            objective: "maximize_mean_log_leave_one_out_event_intensity".into(),
            tie_break: "smallest_bandwidth_um".into(),
            selection_uses_spatial_curve: false,
            selected_index,
            selected_bandwidth_um,
            candidates,
            intensity_evaluations: counters.intensity_evaluations,
            artifact_digest: artifact_digest.to_string(),
        },
        analysis,
        configuration_digest: bandwidth_selection_configuration_digest(config).to_string(),
        estimated_storage_bytes,
        selection_limits: config.selection_limits,
    })
}

fn selection_analysis_limits(
    config: &GaussianBandwidthSelectionConfig,
) -> Result<InhomogeneousSpatialLimits, InhomogeneousSpatialError> {
    let limits = config.analysis_limits;
    InhomogeneousSpatialLimits::new(
        limits.maximum_points,
        limits.maximum_radii,
        limits.maximum_probes,
        config.selection_limits.maximum_intensity_evaluations,
        limits.maximum_pair_visits,
        limits.maximum_null_draws,
        limits.maximum_retained_bytes,
    )
}

pub(super) fn candidate_config(
    config: &GaussianBandwidthSelectionConfig,
    bandwidth_um: f64,
    limits: InhomogeneousSpatialLimits,
) -> Result<InhomogeneousSpatialConfig, InhomogeneousSpatialError> {
    InhomogeneousSpatialConfig::new(
        config.radii_um.to_vec(),
        bandwidth_um,
        config.integration_grid,
        config.simulations,
        config.seed,
        config.alpha,
        config.minimum_intensity_per_um2,
        limits,
    )
}

pub(super) fn selected_retained_bytes(
    analysis: &super::InhomogeneousSpatialResult,
    candidate_count: usize,
) -> Result<usize, InhomogeneousSpatialError> {
    let selection = candidate_count
        .checked_mul(std::mem::size_of::<GaussianBandwidthCandidateScore>())
        .and_then(|value| value.checked_add(256))
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    analysis
        .estimated_storage_bytes
        .checked_add(selection)
        .ok_or(InhomogeneousSpatialError::SizeOverflow)
}

pub(super) fn bandwidth_selection_configuration_digest(
    config: &GaussianBandwidthSelectionConfig,
) -> ContentDigest {
    let mut fields = vec![b"marklab-gaussian-bandwidth-selection-config-v1".to_vec()];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    for bandwidth in &config.candidate_bandwidths_um {
        fields.push(bandwidth.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        (config.integration_grid[0] as u128).to_be_bytes().to_vec(),
        (config.integration_grid[1] as u128).to_be_bytes().to_vec(),
        (config.simulations as u128).to_be_bytes().to_vec(),
        config.seed.to_be_bytes().to_vec(),
        config.alpha.to_bits().to_be_bytes().to_vec(),
        config
            .minimum_intensity_per_um2
            .to_bits()
            .to_be_bytes()
            .to_vec(),
        (config.analysis_limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.analysis_limits.maximum_radii as u128)
            .to_be_bytes()
            .to_vec(),
        (config.analysis_limits.maximum_probes as u128)
            .to_be_bytes()
            .to_vec(),
        (config.analysis_limits.maximum_intensity_evaluations as u128)
            .to_be_bytes()
            .to_vec(),
        (config.analysis_limits.maximum_pair_visits as u128)
            .to_be_bytes()
            .to_vec(),
        (config.analysis_limits.maximum_null_draws as u128)
            .to_be_bytes()
            .to_vec(),
        (config.analysis_limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
        (config.selection_limits.maximum_candidates as u128)
            .to_be_bytes()
            .to_vec(),
        (config.selection_limits.maximum_intensity_evaluations as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

pub(super) fn selection_digest(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &GaussianBandwidthSelectionConfig,
    candidates: &[GaussianBandwidthCandidateScore],
    selected_index: usize,
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-gaussian-bandwidth-selection-result-v1".to_vec(),
        window.descriptor().logical_digest.as_bytes().to_vec(),
        bandwidth_selection_configuration_digest(config)
            .as_bytes()
            .to_vec(),
        (selected_index as u128).to_be_bytes().to_vec(),
    ];
    for row in 0..pattern.len() {
        fields.push(pattern.x_um[row].to_bits().to_be_bytes().to_vec());
        fields.push(pattern.y_um[row].to_bits().to_be_bytes().to_vec());
    }
    for candidate in candidates {
        fields.push(candidate.bandwidth_um.to_bits().to_be_bytes().to_vec());
        fields.push(
            candidate
                .mean_log_leave_one_out_intensity
                .to_bits()
                .to_be_bytes()
                .to_vec(),
        );
        fields.push(
            candidate
                .minimum_intensity_per_um2
                .to_bits()
                .to_be_bytes()
                .to_vec(),
        );
        fields.push(
            candidate
                .maximum_intensity_per_um2
                .to_bits()
                .to_be_bytes()
                .to_vec(),
        );
        fields.push(
            (candidate.intensity_evaluations as u128)
                .to_be_bytes()
                .to_vec(),
        );
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}
