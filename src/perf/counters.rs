use crate::errors::{MarklabError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryInputs {
    pub n_points: usize,
    pub optional_point_bytes: usize,
    pub raster_pixels: usize,
    pub raster_bytes_per_pixel: usize,
    pub active_raster_buffers: usize,
    pub n_shells: usize,
    pub n_outputs: usize,
    pub n_permutations: usize,
    pub n_scalar_stats: usize,
    pub k_chunk_modes: usize,
    pub scratch_per_mode_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryEstimate {
    pub points_bytes: usize,
    pub raster_bytes: usize,
    pub spectrum_bytes: usize,
    pub permutation_summary_bytes: usize,
    pub k_chunk_bytes: usize,
    pub total_bytes: usize,
}

impl MemoryEstimate {
    pub fn total_mib(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn enforce_budget_mib(&self, budget_mib: usize) -> Result<()> {
        let budget_bytes = budget_mib.saturating_mul(1024 * 1024);
        if self.total_bytes > budget_bytes {
            return Err(MarklabError::Validation(format!(
                "estimated peak memory {:.2} MiB exceeds configured budget {budget_mib} MiB",
                self.total_mib()
            )));
        }
        Ok(())
    }
}

pub(crate) fn enforce_storage_budget(
    label: &str,
    required_bytes: usize,
    budget_bytes: usize,
) -> Result<()> {
    if required_bytes > budget_bytes {
        return Err(MarklabError::Validation(format!(
            "estimated {label} storage {required_bytes} bytes exceeds remaining geometry memory budget {budget_bytes} bytes"
        )));
    }
    Ok(())
}

pub fn estimate_peak_memory(inputs: MemoryInputs) -> MemoryEstimate {
    let points_bytes = inputs
        .n_points
        .saturating_mul(8 + 8 + 1 + inputs.optional_point_bytes);
    let raster_bytes = inputs
        .raster_pixels
        .saturating_mul(inputs.raster_bytes_per_pixel)
        .saturating_mul(inputs.active_raster_buffers);
    let spectrum_bytes = inputs
        .n_shells
        .saturating_mul(inputs.n_outputs)
        .saturating_mul(8);
    let permutation_summary_bytes = inputs
        .n_permutations
        .saturating_mul(inputs.n_scalar_stats)
        .saturating_mul(8);
    let k_chunk_bytes = inputs
        .k_chunk_modes
        .saturating_mul(inputs.scratch_per_mode_bytes);
    let total_bytes = points_bytes
        .saturating_add(raster_bytes)
        .saturating_add(spectrum_bytes)
        .saturating_add(permutation_summary_bytes)
        .saturating_add(k_chunk_bytes);

    MemoryEstimate {
        points_bytes,
        raster_bytes,
        spectrum_bytes,
        permutation_summary_bytes,
        k_chunk_bytes,
        total_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_accounts_for_points_rasters_permutations_and_k_chunks() {
        let estimate = estimate_peak_memory(MemoryInputs {
            n_points: 100,
            optional_point_bytes: 5,
            raster_pixels: 1000,
            raster_bytes_per_pixel: 4,
            active_raster_buffers: 3,
            n_shells: 64,
            n_outputs: 4,
            n_permutations: 99,
            n_scalar_stats: 6,
            k_chunk_modes: 16,
            scratch_per_mode_bytes: 32,
        });

        assert!(estimate.total_bytes > estimate.points_bytes);
        assert!(estimate.total_mib() > 0.0);
        assert!(estimate.enforce_budget_mib(1).is_ok());
        assert!(estimate.enforce_budget_mib(0).is_err());
    }

    #[test]
    fn geometry_storage_budget_reports_required_and_available_bytes() {
        enforce_storage_budget("pair plan", 1024, 1024).expect("exact budget");
        let error =
            enforce_storage_budget("pair plan", 1025, 1024).expect_err("over-budget storage");

        assert!(error
            .to_string()
            .contains("estimated pair plan storage 1025"));
        assert!(error
            .to_string()
            .contains("remaining geometry memory budget 1024"));
    }
}
