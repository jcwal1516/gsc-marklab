mod marked;
mod multimodal;

pub(super) use marked::synthetic_pattern;
#[cfg(test)]
pub(super) use marked::{calibration_pattern, CalibrationLabels, CalibrationLayout};
#[cfg(test)]
pub(super) use multimodal::{
    multimodal_calibration_scenario, MultimodalCalibrationLayout, CALIBRATION_NULL_MODELS,
};
pub(super) use multimodal::{
    multimodal_replicate_scenario, multimodal_smoke_config, MultimodalScenario,
};

/// Small seeded generator shared by the synthetic scenarios; not for cryptographic use.
#[derive(Clone, Copy, Debug)]
pub(in crate::synthetic_smoke) struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub(in crate::synthetic_smoke) fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xa076_1d64_78bd_642f,
        }
    }

    pub(in crate::synthetic_smoke) fn unit(&mut self) -> f64 {
        let value = self.next_u64() >> 11;
        value as f64 / ((1_u64 << 53) as f64)
    }

    pub(in crate::synthetic_smoke) fn centered(&mut self, width: f64) -> f64 {
        (2.0 * self.unit() - 1.0) * width
    }

    pub(in crate::synthetic_smoke) fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state ^ (self.state >> 33)
    }
}
