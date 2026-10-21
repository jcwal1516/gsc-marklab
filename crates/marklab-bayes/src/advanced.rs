//! Typed contracts for the existing advanced NumPy/SciPy research worker.

use marklab_core::backend::is_worker_timeout;
use serde::{Deserialize, Serialize};

mod result;
mod spec;
pub mod thomas_cluster;
mod validation;

pub use result::*;
pub use spec::*;
pub use validation::validate_adaptive_scientific_result;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    HmcNormal,
    SpdeSuite,
    AdaptiveWindowSpde,
    NonstationaryAdaptiveWindowSpde,
}

impl Mode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "hmc_normal" => Some(Self::HmcNormal),
            "spde_suite" => Some(Self::SpdeSuite),
            "adaptive_window_spde" => Some(Self::AdaptiveWindowSpde),
            "nonstationary_adaptive_window_spde" => Some(Self::NonstationaryAdaptiveWindowSpde),
            _ => None,
        }
    }

    fn result_identity(self) -> (&'static str, u32) {
        match self {
            Self::HmcNormal => ("marklab.fixed_step_hmc_normal_mean", 1),
            Self::SpdeSuite => ("marklab.rectangular_spde_suite", 1),
            Self::AdaptiveWindowSpde => ("marklab.adaptive_window_spde", 1),
            Self::NonstationaryAdaptiveWindowSpde => {
                ("marklab.nonstationary_adaptive_window_spde", 1)
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum AdvancedSpec {
    HmcNormal(HmcNormalSpec),
    SpdeSuite(RectangularSpdeSpec),
    AdaptiveWindowSpde(AdaptiveWindowSpdeSpec),
    NonstationaryAdaptiveWindowSpde(NonstationaryAdaptiveWindowSpdeSpec),
}

impl AdvancedSpec {
    pub fn decode(mode: Mode, bytes: &[u8]) -> Result<Self, serde_json::Error> {
        match mode {
            Mode::HmcNormal => serde_json::from_slice(bytes).map(Self::HmcNormal),
            Mode::SpdeSuite => serde_json::from_slice(bytes).map(Self::SpdeSuite),
            Mode::AdaptiveWindowSpde => serde_json::from_slice(bytes).map(Self::AdaptiveWindowSpde),
            Mode::NonstationaryAdaptiveWindowSpde => {
                serde_json::from_slice(bytes).map(Self::NonstationaryAdaptiveWindowSpde)
            }
        }
    }

    pub fn timeout_seconds(&self) -> Result<u64, &'static str> {
        let value = match self {
            Self::HmcNormal(spec) => &spec.timeout_seconds,
            Self::SpdeSuite(spec) => &spec.timeout_seconds,
            Self::AdaptiveWindowSpde(spec) => &spec.timeout_seconds,
            Self::NonstationaryAdaptiveWindowSpde(spec) => &spec.timeout_seconds,
        };
        let timeout = value
            .as_u64()
            .ok_or("advanced Bayesian input requires integer timeout_seconds")?;
        if !is_worker_timeout(timeout) {
            return Err("advanced Bayesian timeout must be between 1 and 3600 seconds");
        }
        Ok(timeout)
    }

    pub fn window(&self) -> Option<&MultiPolygon> {
        match self {
            Self::AdaptiveWindowSpde(spec) => Some(&spec.window),
            Self::NonstationaryAdaptiveWindowSpde(spec) => Some(&spec.window),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedRequest {
    pub format: &'static str,
    pub version: u32,
    pub mode: Mode,
    pub backend: Backend,
    pub spec: AdvancedSpec,
    pub input_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_window_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_window_area: Option<f64>,
}

impl AdvancedRequest {
    fn validate_identity(
        &self,
        (format, version, backend, reported_sha256): (&str, u32, &Backend, &str),
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        let expected = self.mode.result_identity();
        if format != expected.0
            || version != expected.1
            || backend != &self.backend
            || reported_sha256 != request_sha256
        {
            return Err("advanced Bayesian result identity mismatch");
        }
        Ok(())
    }

    fn validate_window(&self, digest: &str, area: f64) -> Result<(), &'static str> {
        if self.canonical_window_sha256.as_deref() != Some(digest)
            || self.canonical_window_area != Some(area)
        {
            return Err("advanced Bayesian result identity mismatch");
        }
        Ok(())
    }

    pub fn validate_adaptive(
        &self,
        result: &AdaptiveWindowSpdeResult,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        self.validate_identity(
            (
                &result.format,
                result.version,
                &result.backend,
                &result.request_sha256,
            ),
            request_sha256,
        )?;
        self.validate_window(&result.window.canonical_sha256, result.window.exact_area)
    }

    pub fn validate_nonstationary(
        &self,
        result: &NonstationaryAdaptiveWindowSpdeResult,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        self.validate_identity(
            (
                &result.format,
                result.version,
                &result.backend,
                &result.request_sha256,
            ),
            request_sha256,
        )?;
        self.validate_window(&result.window.canonical_sha256, result.window.exact_area)?;
        let AdvancedSpec::NonstationaryAdaptiveWindowSpde(spec) = &self.spec else {
            return Err("advanced Bayesian result identity mismatch");
        };
        validation::validate_nonstationary_adaptive_result(spec, result)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum AdvancedResult {
    HmcNormal(Box<HmcNormalResult>),
    SpdeSuite(Box<RectangularSpdeResult>),
    AdaptiveWindowSpde(Box<AdaptiveWindowSpdeResult>),
    NonstationaryAdaptiveWindowSpde(Box<NonstationaryAdaptiveWindowSpdeResult>),
}

impl AdvancedResult {
    pub fn decode(mode: Mode, bytes: &[u8]) -> Result<Self, serde_json::Error> {
        match mode {
            Mode::HmcNormal => serde_json::from_slice(bytes).map(Self::HmcNormal),
            Mode::SpdeSuite => serde_json::from_slice(bytes).map(Self::SpdeSuite),
            Mode::AdaptiveWindowSpde => serde_json::from_slice(bytes).map(Self::AdaptiveWindowSpde),
            Mode::NonstationaryAdaptiveWindowSpde => {
                serde_json::from_slice(bytes).map(Self::NonstationaryAdaptiveWindowSpde)
            }
        }
    }

    pub fn validate(
        &self,
        request: &AdvancedRequest,
        request_sha256: &str,
    ) -> Result<(), &'static str> {
        let identity = match self {
            Self::HmcNormal(result) => (
                &result.format,
                result.version,
                &result.backend,
                &result.request_sha256,
            ),
            Self::SpdeSuite(result) => (
                &result.format,
                result.version,
                &result.backend,
                &result.request_sha256,
            ),
            Self::AdaptiveWindowSpde(result) => {
                return request.validate_adaptive(result, request_sha256)
            }
            Self::NonstationaryAdaptiveWindowSpde(result) => {
                return request.validate_nonstationary(result, request_sha256)
            }
        };
        request.validate_identity(
            (identity.0, identity.1, identity.2, identity.3),
            request_sha256,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_request_encoding_preserves_numeric_forms_and_optional_fields() {
        let input = br#"{
            "observations":[1,1.0],"observation_standard_deviation":0.5,
            "prior_mean":0,"prior_standard_deviation":2.0,"initial_state":0.0,
            "mass":1,"step_size":0.05,"leapfrog_steps":20,
            "warmup":500,"draws":2000,"seed":307,"timeout_seconds":60
        }"#;
        let spec = AdvancedSpec::decode(Mode::HmcNormal, input).unwrap();
        assert_eq!(spec.timeout_seconds().unwrap(), 60);
        let request = AdvancedRequest {
            format: "marklab.scipy_advanced_bayes_request",
            version: 1,
            mode: Mode::HmcNormal,
            backend: Backend {
                name: "numpy_scipy_advanced_bayes".into(),
                scipy_version: "scipy".into(),
                numpy_version: "numpy".into(),
                python_version: "python".into(),
                license: "BSD-3-Clause".into(),
                environment_lock_sha256: "lock".into(),
                worker_sha256: "worker".into(),
            },
            spec,
            input_sha256: "input".into(),
            canonical_window_sha256: None,
            canonical_window_area: None,
        };
        let legacy = serde_json::json!({
            "format":"marklab.scipy_advanced_bayes_request","version":1,"mode":"hmc_normal",
            "backend":request.backend,"spec":serde_json::from_slice::<serde_json::Value>(input).unwrap(),
            "input_sha256":"input"
        });
        assert_eq!(
            serde_json::to_vec(&serde_json::to_value(&request).unwrap()).unwrap(),
            serde_json::to_vec(&legacy).unwrap()
        );

        let mut rectangular = serde_json::json!({
            "window":{"frame":"square","bounds":[0,1.0,0.0,1],"holes":[]},
            "mesh_resolutions":[5,7],"alpha":2,"kappa":3.0,"tau":0.2,
            "lgcp_points":[[0,0.5]],"lgcp_beta_prior_sd":3.0,
            "factor_observations":[{"region_id":"region","coordinates":[0,0.5],"values":[1,1.0]}],
            "factor_noise_sd":0.08,"maximum_iterations":1000,"timeout_seconds":60
        });
        for factor_count in [None, Some(serde_json::json!(1))] {
            if let Some(factor_count) = factor_count {
                rectangular["factor_count"] = factor_count;
            }
            let bytes = serde_json::to_vec(&rectangular).unwrap();
            let typed = AdvancedSpec::decode(Mode::SpdeSuite, &bytes).unwrap();
            assert_eq!(
                serde_json::to_vec(&serde_json::to_value(typed).unwrap()).unwrap(),
                bytes
            );
        }
        rectangular["window"]["unrecognized_geometry"] = serde_json::json!(true);
        assert!(
            AdvancedSpec::decode(Mode::SpdeSuite, &serde_json::to_vec(&rectangular).unwrap())
                .is_err()
        );
    }

    #[test]
    fn advanced_results_reject_unknown_and_malformed_nested_fields() {
        let mut response = serde_json::json!({
            "format":"marklab.fixed_step_hmc_normal_mean","version":1,
            "algorithm":{"mass":1.0,"step_size":0.05,"leapfrog_steps":20,"warmup":500,"draws":2,"seed":307},
            "posterior":{"mean":2.0,"standard_deviation":0.1,"interval_95":[1.8,2.2],"draws":[1.9,2.1]},
            "analytic_posterior":{"mean":2.0,"standard_deviation":0.1},
            "diagnostics":{"acceptance_rate":1.0,"maximum_absolute_energy_error":0.1,"mean_energy_error":0.0,"constraint_transform_jacobian":0.0},
            "claim_status":"experimental_synthetic_fixed_step_hmc",
            "backend":{"name":"worker","scipy_version":"scipy","numpy_version":"numpy","python_version":"python","license":"BSD-3-Clause","environment_lock_sha256":"lock","worker_sha256":"worker"},
            "request_sha256":"request"
        });
        let result =
            AdvancedResult::decode(Mode::HmcNormal, &serde_json::to_vec(&response).unwrap())
                .unwrap();
        assert_eq!(serde_json::to_value(result).unwrap(), response);
        response["posterior"]["unrecognized_interval"] = serde_json::json!([0, 1]);
        assert!(
            AdvancedResult::decode(Mode::HmcNormal, &serde_json::to_vec(&response).unwrap())
                .is_err()
        );
        response["posterior"]
            .as_object_mut()
            .unwrap()
            .remove("unrecognized_interval");
        response["posterior"]["interval_95"] = serde_json::json!([1.8, 2.0, 2.2]);
        assert!(
            AdvancedResult::decode(Mode::HmcNormal, &serde_json::to_vec(&response).unwrap())
                .is_err()
        );
    }
}
