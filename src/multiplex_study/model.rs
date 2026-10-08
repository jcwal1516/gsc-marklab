use std::collections::BTreeMap;

use marklab_policy::Maturity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recipe {
    pub format: String,
    pub version: u32,
    pub study_id: String,
    pub channels: Vec<Channel>,
    pub design: Design,
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Channel {
    pub id: String,
    pub label: String,
    pub kind: ChannelKind,
    pub unit: String,
    pub measurement_status: String,
    pub provenance: String,
    #[serde(default)]
    pub levels: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ChannelKind {
    Continuous,
    Binary,
    Categorical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Design {
    pub selected_channels: Vec<String>,
    pub radius_um: f64,
    pub weight_policy: String,
    pub missingness: String,
    pub patient_reduction: String,
    pub exchangeability: String,
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Slide {
    pub slide_id: String,
    pub patient_id: String,
    pub group: String,
    pub coordinate_frame_id: String,
    pub window: serde_json::Value,
    pub cell_ids: Vec<String>,
    pub coordinates_um: Vec<[f64; 2]>,
    pub observations: BTreeMap<String, Vec<Option<f64>>>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Limits {
    pub maximum_rows_per_slide: usize,
    pub maximum_total_rows: usize,
    pub maximum_directed_edges: usize,
    pub maximum_edge_evaluations: usize,
    pub maximum_memory_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            maximum_rows_per_slide: 100_000,
            maximum_total_rows: 200_000,
            maximum_directed_edges: 1_000_000,
            maximum_edge_evaluations: 50_000_000,
            maximum_memory_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Version-one, claim-bounded scientific result of a complete multiplex cohort recipe.
///
/// Serialize with Serde to obtain the documented study JSON profile. This envelope is separate
/// from result-format 0.3. It retains every admitted slide and patient, including unavailable ones.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiplexStudyResult {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) study_id: String,
    pub(super) recipe_sha256: String,
    pub(super) channels: Vec<Channel>,
    pub(super) design: Design,
    pub(super) limits: Limits,
    pub(super) endpoint_names: Vec<String>,
    pub(super) slides: Vec<SlideResult>,
    pub(super) patients: Vec<PatientResult>,
    pub(super) inference: InferenceResult,
    pub(super) maturity: MaturityDescription,
    pub(super) claim_scope: String,
    pub(super) evidence_limits: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SlideResult {
    pub slide_id: String,
    pub patient_id: String,
    pub group: String,
    pub coordinate_frame_id: String,
    pub source_sha256: String,
    pub panel_identity: String,
    pub window_identity: String,
    pub cell_count: usize,
    pub channels: Vec<ChannelResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChannelResult {
    pub channel: String,
    pub status: String,
    pub reason: Option<String>,
    pub observed_rows: usize,
    pub missing_rows: usize,
    pub directed_edges: usize,
    pub weights_digest: Option<String>,
    pub moran_i: Option<f64>,
    pub geary_c: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PatientResult {
    pub patient_id: String,
    pub group: String,
    pub slide_count: usize,
    pub slide_ids: Vec<String>,
    pub status: String,
    pub values: Option<Vec<f64>>,
    pub unavailable_slide_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InferenceResult {
    pub status: String,
    pub reason: Option<String>,
    pub correction: String,
    pub endpoints: Vec<EndpointResult>,
    pub critical_value: Option<f64>,
    pub permutations_attempted: Option<usize>,
    pub permutations_completed: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EndpointResult {
    pub endpoint: String,
    pub effect_group_a_minus_group_b: f64,
    pub studentized_statistic: f64,
    pub adjusted_p_value: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MaturityDescription {
    pub format: String,
    pub version: u32,
    pub method_maturity: Maturity,
    pub result_maturity: Maturity,
    pub downgraded: bool,
    pub reasons: Vec<String>,
    pub policy: String,
}

impl MultiplexStudyResult {
    pub(super) fn validate_claim_state(&self) -> crate::Result<()> {
        use super::invalid;
        if self.format != "marklab.multiplex_study_result"
            || self.version != 1
            || self.design.permutations == 0
            || self.patients.is_empty()
            || self.endpoint_names.len() != self.design.selected_channels.len() * 2
        {
            return Err(invalid("invalid study result profile or endpoint family"));
        }
        for patient in &self.patients {
            if let Some(values) = &patient.values {
                if values.len() != self.endpoint_names.len()
                    || values.iter().any(|value| !value.is_finite())
                {
                    return Err(invalid("patient result has invalid endpoint values"));
                }
            }
        }
        let inference = &self.inference;
        let available = inference.status == "available";
        if self.maturity.method_maturity != marklab_policy::Maturity::Experimental
            || self.maturity.result_maturity
                != if available {
                    marklab_policy::Maturity::Experimental
                } else {
                    marklab_policy::Maturity::UnsupportedForClaim
                }
            || inference.correction != "single_step_max_t"
        {
            return Err(invalid(
                "cached study cannot promote its actual evidence or change its correction",
            ));
        }
        if available {
            if self.patients.iter().any(|patient| patient.values.is_none())
                || inference.reason.is_some()
                || inference.endpoints.len() != self.endpoint_names.len()
                || inference.permutations_attempted != Some(self.design.permutations)
                || inference.permutations_completed != Some(self.design.permutations)
                || !inference
                    .critical_value
                    .is_some_and(|value| value.is_finite() && value >= 0.0)
            {
                return Err(invalid("cached available inference is incomplete"));
            }
            for (row, name) in inference.endpoints.iter().zip(&self.endpoint_names) {
                if &row.endpoint != name
                    || !row.effect_group_a_minus_group_b.is_finite()
                    || !row.studentized_statistic.is_finite()
                    || !row.adjusted_p_value.is_finite()
                    || !(0.0..=1.0).contains(&row.adjusted_p_value)
                {
                    return Err(invalid("cached inference endpoint is invalid"));
                }
            }
        } else if inference.status != "unavailable"
            || inference.reason.is_none()
            || !inference.endpoints.is_empty()
            || inference.critical_value.is_some()
        {
            return Err(invalid(
                "cached unavailable inference has a contradictory payload",
            ));
        }
        if available
            && self.slides.iter().any(|slide| {
                slide.channels.iter().any(|channel| {
                    channel.status != "available"
                        || !channel.moran_i.is_some_and(f64::is_finite)
                        || !channel.geary_c.is_some_and(f64::is_finite)
                })
            })
        {
            return Err(invalid(
                "an unavailable slide cannot support an available inference",
            ));
        }
        Ok(())
    }
}
