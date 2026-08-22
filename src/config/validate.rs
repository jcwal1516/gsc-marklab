use std::collections::BTreeSet;

use crate::errors::{MarklabError, Result};

use super::model::{AnalysisConfig, ThreadSetting};

impl AnalysisConfig {
    pub fn validate(&self) -> Result<()> {
        if self.analysis.mark_label.trim().is_empty() {
            return config_error("analysis.mark_label must not be empty");
        }
        if self.validation.n_min == 0 {
            return config_error("validation.n_min must be greater than zero");
        }
        if self.validation.n_marked_min + self.validation.n_unmarked_min > self.validation.n_min {
            return config_error(
                "validation.n_marked_min + validation.n_unmarked_min must not exceed validation.n_min",
            );
        }
        if !unit_interval_open(self.validation.p_min)
            || !unit_interval_open(self.validation.p_max)
            || self.validation.p_min >= self.validation.p_max
        {
            return config_error(
                "validation.p_min and validation.p_max must be finite, inside (0, 1), and p_min < p_max",
            );
        }
        positive_finite("validation.area_min_um2", self.validation.area_min_um2)?;
        if self.validation.k_shell_min == 0 {
            return config_error("validation.k_shell_min must be greater than zero");
        }
        if !unit_interval_open(self.validation.largest_interpretable_scale_fraction) {
            return config_error(
                "validation.largest_interpretable_scale_fraction must be finite and inside (0, 1)",
            );
        }
        if !unit_interval_closed(self.validation.valid_mask_fraction_min) {
            return config_error(
                "validation.valid_mask_fraction_min must be finite and inside (0, 1]",
            );
        }
        if self.spectrum.k_shells == 0
            || self.spectrum.low_k_shells == 0
            || self.spectrum.anisotropy_low_k_shells == 0
        {
            return config_error("spectrum shell counts must be greater than zero");
        }
        if self.spectrum.low_k_shells > self.spectrum.k_shells
            || self.spectrum.anisotropy_low_k_shells > self.spectrum.k_shells
            || self.validation.k_shell_min > self.spectrum.k_shells
        {
            return config_error("spectrum shell subsets must not exceed spectrum.k_shells");
        }
        if self.multiscale_residual.enabled {
            positive_finite(
                "multiscale_residual.min_territory_z",
                self.multiscale_residual.min_territory_z,
            )?;
        }
        if !unit_interval_open(self.inference.family_wise_alpha) {
            return config_error("inference.family_wise_alpha must be finite and inside (0, 1)");
        }
        let n_curves = self.permutation.b.saturating_add(1);
        if n_curves as f64 * self.inference.family_wise_alpha < 1.0 {
            return config_error("permutation requires (B + 1) * alpha >= 1");
        }
        if self.spectrum.fit_low_k_alpha
            && (n_curves as f64) < 2.0 / self.inference.family_wise_alpha
        {
            return config_error("equal-tail endpoints require B + 1 >= 2 / alpha");
        }
        if self.permutation.stratified && self.permutation.strata_fields.is_empty() {
            return config_error(
                "permutation.strata_fields must not be empty when stratified is true",
            );
        }
        if self.analysis.use_probabilistic_marks && self.permutation.stratified {
            return config_error(
                "analysis.use_probabilistic_marks is not supported with stratified permutation",
            );
        }
        reject_duplicates("permutation.strata_fields", &self.permutation.strata_fields)?;

        if self.registration.enabled {
            if self.registration.min_landmarks == 0 {
                return config_error("registration.min_landmarks must be greater than zero");
            }
            nonnegative_finite("registration.max_rmse_um", self.registration.max_rmse_um)?;
            positive_finite(
                "registration.claim_distance_multiplier",
                self.registration.claim_distance_multiplier,
            )?;
        }
        if self.neighborhood.enabled {
            positive_finite("neighborhood.radius_um", self.neighborhood.radius_um)?;
            positive_finite(
                "neighborhood.territory_eps_um",
                self.neighborhood.territory_eps_um,
            )?;
            if self.neighborhood.territory_min_cells == 0 {
                return config_error("neighborhood.territory_min_cells must be greater than zero");
            }
            positive_finite(
                "neighborhood.territory_min_radius_um",
                self.neighborhood.territory_min_radius_um,
            )?;
            reject_duplicates("neighborhood.null_models", &self.neighborhood.null_models)?;
        }
        for (field, margin) in [
            ("spectrum", self.comparison.margins.spectrum),
            (
                "mark_pair_covariance",
                self.comparison.margins.mark_pair_covariance,
            ),
            (
                "cross_interaction",
                self.comparison.margins.cross_interaction,
            ),
            (
                "graph_enrichment_log2",
                self.comparison.margins.graph_enrichment_log2,
            ),
            (
                "territory_profile",
                self.comparison.margins.territory_profile,
            ),
        ] {
            if let Some(value) = margin {
                nonnegative_finite(&format!("comparison.margins.{field}"), value)?;
            }
        }
        if matches!(self.performance.threads, ThreadSetting::Count(0)) {
            return config_error("performance.threads must be 'auto' or a positive integer");
        }
        if self.performance.memory_budget_mib == 0 || self.performance.k_chunk_modes == 0 {
            return config_error(
                "performance.memory_budget_mib and performance.k_chunk_modes must be greater than zero",
            );
        }
        Ok(())
    }
}

fn config_error<T>(message: impl Into<String>) -> Result<T> {
    Err(MarklabError::Config(message.into()))
}

fn positive_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        config_error(format!("{field} must be finite and positive"))
    }
}

fn nonnegative_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        config_error(format!("{field} must be finite and non-negative"))
    }
}

fn unit_interval_open(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

fn unit_interval_closed(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value <= 1.0
}

fn reject_duplicates<T: Ord>(field: &str, values: &[T]) -> Result<()> {
    let mut unique = BTreeSet::new();
    if values.iter().any(|value| !unique.insert(value)) {
        config_error(format!("{field} must not contain duplicates"))
    } else {
        Ok(())
    }
}
