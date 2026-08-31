use marklab::PatientId;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use super::{
    execute as execute_serial, prepare_spec as prepare_serial, PreparedRegisteredSerialVoxelK,
    RegisteredSerialVoxelKResult, RegisteredSerialVoxelKSpec,
};

const MINIMUM_DEFORMATION_DRAWS: usize = 20;
const MAXIMUM_DEFORMATION_DRAWS: usize = 2_000;
const MAXIMUM_RADIUS_DRAW_EVALUATIONS: u64 = 2_000_000;
const MAXIMUM_MEMORY_MIB: usize = 4_096;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TreatmentInterval {
    interval_id: String,
    treatment_or_exposure_id: String,
    start_day: f64,
    end_day: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisteredLongitudinalVoxelKSpec {
    patient_id: String,
    lesion_id: String,
    anatomical_site: String,
    baseline_elapsed_days: f64,
    follow_up_elapsed_days: f64,
    treatment_interval: TreatmentInterval,
    cross_time_registration_id: String,
    deformation_posterior_id: String,
    baseline: RegisteredSerialVoxelKSpec,
    follow_up: RegisteredSerialVoxelKSpec,
    deformation_only_delta_k_draws_um3: Vec<Vec<f64>>,
    negative_control_delta_k_um3: Vec<f64>,
    independent_change_delta_k_um3: Vec<f64>,
    maximum_deformation_draws: usize,
    maximum_radius_draw_evaluations: u64,
    memory_budget_mib: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedRegisteredLongitudinalVoxelK {
    patient_id: String,
    lesion_id: String,
    anatomical_site: String,
    baseline_elapsed_days: f64,
    follow_up_elapsed_days: f64,
    treatment_interval: TreatmentInterval,
    cross_time_registration_id: String,
    deformation_posterior_id: String,
    baseline: PreparedRegisteredSerialVoxelK,
    follow_up: PreparedRegisteredSerialVoxelK,
    deformation_draws: Vec<Vec<f64>>,
    negative_control: Vec<f64>,
    independent_change: Vec<f64>,
    maximum_deformation_draws: usize,
    radius_draw_evaluations: u64,
    maximum_radius_draw_evaluations: u64,
    retained_memory_bytes: usize,
    memory_budget_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RegisteredLongitudinalCurvePoint {
    radius_um: f64,
    baseline_k_um3: f64,
    follow_up_k_um3: f64,
    observed_delta_k_um3: f64,
    deformation_only_delta_mean_um3: f64,
    deformation_only_lower_95_um3: f64,
    deformation_only_upper_95_um3: f64,
    registration_adjusted_delta_mean_um3: f64,
    registration_adjusted_lower_95_um3: f64,
    registration_adjusted_upper_95_um3: f64,
    negative_control_delta_k_um3: f64,
    independent_change_delta_k_um3: f64,
    adjusted_interval_excludes_zero: bool,
    negative_control_compatible_with_registration: bool,
    independent_direction_agrees: bool,
    change_supported_beyond_registration: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RegisteredLongitudinalVoxelKResult {
    format: &'static str,
    version: u32,
    patient_id: String,
    lesion_id: String,
    anatomical_site: String,
    baseline_specimen_id: String,
    follow_up_specimen_id: String,
    baseline_timepoint_id: String,
    follow_up_timepoint_id: String,
    baseline_elapsed_days: f64,
    follow_up_elapsed_days: f64,
    treatment_interval: TreatmentInterval,
    cross_time_registration_id: String,
    deformation_posterior_id: String,
    deformation_draw_count: usize,
    maximum_deformation_draws: usize,
    baseline: RegisteredSerialVoxelKResult,
    follow_up: RegisteredSerialVoxelKResult,
    curve: Vec<RegisteredLongitudinalCurvePoint>,
    radius_draw_evaluations: u64,
    maximum_radius_draw_evaluations: u64,
    retained_memory_bytes: usize,
    memory_budget_bytes: usize,
    statistical_unit: &'static str,
    null_model: &'static str,
    finite_result_policy: &'static str,
    assumptions: [&'static str; 4],
    claim_status: &'static str,
}

impl<'de> Deserialize<'de> for RegisteredLongitudinalVoxelKResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Owned {
            format: String,
            version: u32,
            patient_id: String,
            lesion_id: String,
            anatomical_site: String,
            baseline_specimen_id: String,
            follow_up_specimen_id: String,
            baseline_timepoint_id: String,
            follow_up_timepoint_id: String,
            baseline_elapsed_days: f64,
            follow_up_elapsed_days: f64,
            treatment_interval: TreatmentInterval,
            cross_time_registration_id: String,
            deformation_posterior_id: String,
            deformation_draw_count: usize,
            maximum_deformation_draws: usize,
            baseline: RegisteredSerialVoxelKResult,
            follow_up: RegisteredSerialVoxelKResult,
            curve: Vec<RegisteredLongitudinalCurvePoint>,
            radius_draw_evaluations: u64,
            maximum_radius_draw_evaluations: u64,
            retained_memory_bytes: usize,
            memory_budget_bytes: usize,
            statistical_unit: String,
            null_model: String,
            finite_result_policy: String,
            assumptions: [String; 4],
            claim_status: String,
        }
        let owned = Owned::deserialize(deserializer)?;
        let assumptions = assumptions();
        if owned.format != "marklab.registered_longitudinal_voxel_k_change"
            || owned.statistical_unit != "one_patient_lesion_observed_at_two_biopsy_timepoints"
            || owned.null_model
                != "observed_k_change_is_explainable_by_supplied_registration_deformation_draws"
            || owned.finite_result_policy != "reject_non_finite_or_undefined_no_infinity_persisted"
            || owned.assumptions != assumptions
            || owned.claim_status != "paired_registered_change_diagnostic_not_causal_or_same_cell"
        {
            return Err(serde::de::Error::custom(
                "unexpected registered longitudinal voxel K result identity",
            ));
        }
        Ok(Self {
            format: "marklab.registered_longitudinal_voxel_k_change",
            version: owned.version,
            patient_id: owned.patient_id,
            lesion_id: owned.lesion_id,
            anatomical_site: owned.anatomical_site,
            baseline_specimen_id: owned.baseline_specimen_id,
            follow_up_specimen_id: owned.follow_up_specimen_id,
            baseline_timepoint_id: owned.baseline_timepoint_id,
            follow_up_timepoint_id: owned.follow_up_timepoint_id,
            baseline_elapsed_days: owned.baseline_elapsed_days,
            follow_up_elapsed_days: owned.follow_up_elapsed_days,
            treatment_interval: owned.treatment_interval,
            cross_time_registration_id: owned.cross_time_registration_id,
            deformation_posterior_id: owned.deformation_posterior_id,
            deformation_draw_count: owned.deformation_draw_count,
            maximum_deformation_draws: owned.maximum_deformation_draws,
            baseline: owned.baseline,
            follow_up: owned.follow_up,
            curve: owned.curve,
            radius_draw_evaluations: owned.radius_draw_evaluations,
            maximum_radius_draw_evaluations: owned.maximum_radius_draw_evaluations,
            retained_memory_bytes: owned.retained_memory_bytes,
            memory_budget_bytes: owned.memory_budget_bytes,
            statistical_unit: "one_patient_lesion_observed_at_two_biopsy_timepoints",
            null_model:
                "observed_k_change_is_explainable_by_supplied_registration_deformation_draws",
            finite_result_policy: "reject_non_finite_or_undefined_no_infinity_persisted",
            assumptions,
            claim_status: "paired_registered_change_diagnostic_not_causal_or_same_cell",
        })
    }
}

#[derive(Debug, Error)]
pub(crate) enum RegisteredLongitudinalVoxelKError {
    #[error("invalid registered longitudinal 3-D input: {0}")]
    Invalid(String),
    #[error("invalid registered longitudinal 3-D JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn prepare(
    bytes: &[u8],
) -> Result<PreparedRegisteredLongitudinalVoxelK, RegisteredLongitudinalVoxelKError> {
    let spec: RegisteredLongitudinalVoxelKSpec = serde_json::from_slice(bytes)?;
    let declared_patient = PatientId::new(&spec.patient_id).map_err(invalid)?;
    validate_text(&spec.lesion_id, "lesion_id")?;
    validate_text(&spec.anatomical_site, "anatomical_site")?;
    validate_text(
        &spec.cross_time_registration_id,
        "cross_time_registration_id",
    )?;
    validate_text(&spec.deformation_posterior_id, "deformation_posterior_id")?;
    validate_text(&spec.treatment_interval.interval_id, "interval_id")?;
    validate_text(
        &spec.treatment_interval.treatment_or_exposure_id,
        "treatment_or_exposure_id",
    )?;
    if ![
        spec.baseline_elapsed_days,
        spec.follow_up_elapsed_days,
        spec.treatment_interval.start_day,
        spec.treatment_interval.end_day,
    ]
    .into_iter()
    .all(f64::is_finite)
        || spec.baseline_elapsed_days >= spec.follow_up_elapsed_days
        || spec.treatment_interval.start_day < spec.baseline_elapsed_days
        || spec.treatment_interval.start_day > spec.treatment_interval.end_day
        || spec.treatment_interval.end_day > spec.follow_up_elapsed_days
    {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(
            "biopsy times must increase and the treatment interval must lie between them".into(),
        ));
    }

    let baseline = prepare_serial(spec.baseline).map_err(invalid)?;
    let follow_up = prepare_serial(spec.follow_up).map_err(invalid)?;
    if baseline.patient_id != declared_patient.as_str()
        || follow_up.patient_id != declared_patient.as_str()
    {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(
            "baseline and follow-up must belong to the declared patient".into(),
        ));
    }
    if baseline.anatomical_site != spec.anatomical_site
        || follow_up.anatomical_site != spec.anatomical_site
    {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(
            "baseline and follow-up must use the declared anatomical site".into(),
        ));
    }
    if baseline.timepoint_id == follow_up.timepoint_id
        || baseline.specimen_id == follow_up.specimen_id
    {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(
            "longitudinal analysis requires distinct specimen and timepoint identities".into(),
        ));
    }
    let radii_match = baseline.analysis.radii_um.len() == follow_up.analysis.radii_um.len()
        && baseline
            .analysis
            .radii_um
            .iter()
            .zip(&follow_up.analysis.radii_um)
            .all(|(left, right)| left.to_bits() == right.to_bits());
    if !radii_match
        || baseline.analysis.correction != follow_up.analysis.correction
        || baseline.analysis.anisotropy_matrix != follow_up.analysis.anisotropy_matrix
    {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(
            "baseline and follow-up require identical radii, correction, and anisotropic metric"
                .into(),
        ));
    }
    let radius_count = baseline.analysis.radii_um.len();
    let draw_count = spec.deformation_only_delta_k_draws_um3.len();
    let valid_draws = spec
        .deformation_only_delta_k_draws_um3
        .iter()
        .all(|draw| draw.len() == radius_count && draw.iter().all(|value| value.is_finite()));
    if !(MINIMUM_DEFORMATION_DRAWS..=MAXIMUM_DEFORMATION_DRAWS).contains(&draw_count)
        || spec.maximum_deformation_draws < draw_count
        || spec.maximum_deformation_draws > MAXIMUM_DEFORMATION_DRAWS
        || !valid_draws
        || spec.negative_control_delta_k_um3.len() != radius_count
        || spec
            .negative_control_delta_k_um3
            .iter()
            .any(|value| !value.is_finite())
        || spec.independent_change_delta_k_um3.len() != radius_count
        || spec
            .independent_change_delta_k_um3
            .iter()
            .any(|value| !value.is_finite())
    {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(
            "longitudinal change requires bounded finite deformation draws and aligned negative-control and independent-measurement curves"
                .into(),
        ));
    }
    let radius_draw_evaluations = (draw_count as u64)
        .checked_mul(radius_count as u64)
        .ok_or_else(|| invalid("radius-draw work overflowed"))?;
    if spec.maximum_radius_draw_evaluations == 0
        || spec.maximum_radius_draw_evaluations > MAXIMUM_RADIUS_DRAW_EVALUATIONS
        || radius_draw_evaluations > spec.maximum_radius_draw_evaluations
    {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(format!(
            "radius-draw evaluations {radius_draw_evaluations} exceed caller or built-in bounds"
        )));
    }
    if spec.memory_budget_mib == 0 || spec.memory_budget_mib > MAXIMUM_MEMORY_MIB {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(
            "longitudinal memory budget must be positive and bounded".into(),
        ));
    }
    let memory_budget_bytes = spec
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| invalid("memory budget overflowed"))?;
    let retained_memory_bytes = usize::try_from(radius_draw_evaluations)
        .ok()
        .and_then(|work| work.checked_mul(16))
        .and_then(|bytes| bytes.checked_add(radius_count.checked_mul(512)?))
        .ok_or_else(|| invalid("retained-memory estimate overflowed"))?;
    if retained_memory_bytes > memory_budget_bytes {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(format!(
            "retained-memory estimate {retained_memory_bytes} exceeds caller budget {memory_budget_bytes}"
        )));
    }
    Ok(PreparedRegisteredLongitudinalVoxelK {
        patient_id: declared_patient.as_str().to_owned(),
        lesion_id: spec.lesion_id,
        anatomical_site: spec.anatomical_site,
        baseline_elapsed_days: spec.baseline_elapsed_days,
        follow_up_elapsed_days: spec.follow_up_elapsed_days,
        treatment_interval: spec.treatment_interval,
        cross_time_registration_id: spec.cross_time_registration_id,
        deformation_posterior_id: spec.deformation_posterior_id,
        baseline,
        follow_up,
        deformation_draws: spec.deformation_only_delta_k_draws_um3,
        negative_control: spec.negative_control_delta_k_um3,
        independent_change: spec.independent_change_delta_k_um3,
        maximum_deformation_draws: spec.maximum_deformation_draws,
        radius_draw_evaluations,
        maximum_radius_draw_evaluations: spec.maximum_radius_draw_evaluations,
        retained_memory_bytes,
        memory_budget_bytes,
    })
}

pub(crate) fn execute(
    prepared: &PreparedRegisteredLongitudinalVoxelK,
) -> Result<RegisteredLongitudinalVoxelKResult, RegisteredLongitudinalVoxelKError> {
    let baseline = execute_serial(&prepared.baseline).map_err(invalid)?;
    let follow_up = execute_serial(&prepared.follow_up).map_err(invalid)?;
    let curve = build_curve(prepared, &baseline, &follow_up)?;
    Ok(RegisteredLongitudinalVoxelKResult {
        format: "marklab.registered_longitudinal_voxel_k_change",
        version: 1,
        patient_id: prepared.patient_id.clone(),
        lesion_id: prepared.lesion_id.clone(),
        anatomical_site: prepared.anatomical_site.clone(),
        baseline_specimen_id: prepared.baseline.specimen_id.clone(),
        follow_up_specimen_id: prepared.follow_up.specimen_id.clone(),
        baseline_timepoint_id: prepared.baseline.timepoint_id.clone(),
        follow_up_timepoint_id: prepared.follow_up.timepoint_id.clone(),
        baseline_elapsed_days: prepared.baseline_elapsed_days,
        follow_up_elapsed_days: prepared.follow_up_elapsed_days,
        treatment_interval: prepared.treatment_interval.clone(),
        cross_time_registration_id: prepared.cross_time_registration_id.clone(),
        deformation_posterior_id: prepared.deformation_posterior_id.clone(),
        deformation_draw_count: prepared.deformation_draws.len(),
        maximum_deformation_draws: prepared.maximum_deformation_draws,
        baseline,
        follow_up,
        curve,
        radius_draw_evaluations: prepared.radius_draw_evaluations,
        maximum_radius_draw_evaluations: prepared.maximum_radius_draw_evaluations,
        retained_memory_bytes: prepared.retained_memory_bytes,
        memory_budget_bytes: prepared.memory_budget_bytes,
        statistical_unit: "one_patient_lesion_observed_at_two_biopsy_timepoints",
        null_model: "observed_k_change_is_explainable_by_supplied_registration_deformation_draws",
        finite_result_policy: "reject_non_finite_or_undefined_no_infinity_persisted",
        assumptions: assumptions(),
        claim_status: "paired_registered_change_diagnostic_not_causal_or_same_cell",
    })
}

impl RegisteredLongitudinalVoxelKResult {
    pub(crate) fn validate_for_prepared(
        &self,
        prepared: &PreparedRegisteredLongitudinalVoxelK,
    ) -> Result<(), RegisteredLongitudinalVoxelKError> {
        self.baseline
            .validate_for_prepared(&prepared.baseline)
            .map_err(invalid)?;
        self.follow_up
            .validate_for_prepared(&prepared.follow_up)
            .map_err(invalid)?;
        let expected_curve = build_curve(prepared, &self.baseline, &self.follow_up)?;
        if self.version != 1
            || self.patient_id != prepared.patient_id
            || self.lesion_id != prepared.lesion_id
            || self.anatomical_site != prepared.anatomical_site
            || self.baseline_specimen_id != prepared.baseline.specimen_id
            || self.follow_up_specimen_id != prepared.follow_up.specimen_id
            || self.baseline_timepoint_id != prepared.baseline.timepoint_id
            || self.follow_up_timepoint_id != prepared.follow_up.timepoint_id
            || self.baseline_elapsed_days.to_bits() != prepared.baseline_elapsed_days.to_bits()
            || self.follow_up_elapsed_days.to_bits() != prepared.follow_up_elapsed_days.to_bits()
            || self.treatment_interval != prepared.treatment_interval
            || self.cross_time_registration_id != prepared.cross_time_registration_id
            || self.deformation_posterior_id != prepared.deformation_posterior_id
            || self.deformation_draw_count != prepared.deformation_draws.len()
            || self.maximum_deformation_draws != prepared.maximum_deformation_draws
            || self.curve != expected_curve
            || self.radius_draw_evaluations != prepared.radius_draw_evaluations
            || self.maximum_radius_draw_evaluations != prepared.maximum_radius_draw_evaluations
            || self.retained_memory_bytes != prepared.retained_memory_bytes
            || self.memory_budget_bytes != prepared.memory_budget_bytes
        {
            return Err(RegisteredLongitudinalVoxelKError::Invalid(
                "restored registered longitudinal result differs from its request identity or diagnostics"
                    .into(),
            ));
        }
        Ok(())
    }
}

fn build_curve(
    prepared: &PreparedRegisteredLongitudinalVoxelK,
    baseline: &RegisteredSerialVoxelKResult,
    follow_up: &RegisteredSerialVoxelKResult,
) -> Result<Vec<RegisteredLongitudinalCurvePoint>, RegisteredLongitudinalVoxelKError> {
    baseline
        .analysis
        .curve
        .iter()
        .zip(&follow_up.analysis.curve)
        .enumerate()
        .map(|(radius_index, (baseline_row, follow_up_row))| {
            let observed = follow_up_row.k_um3 - baseline_row.k_um3;
            let mut deformation = prepared
                .deformation_draws
                .iter()
                .map(|draw| draw[radius_index])
                .collect::<Vec<_>>();
            deformation.sort_by(f64::total_cmp);
            let adjusted = deformation
                .iter()
                .map(|value| observed - value)
                .collect::<Vec<_>>();
            let deformation_mean = mean(&deformation);
            let adjusted_mean = mean(&adjusted);
            let deformation_lower = quantile(&deformation, 0.025);
            let deformation_upper = quantile(&deformation, 0.975);
            let mut adjusted_sorted = adjusted;
            adjusted_sorted.sort_by(f64::total_cmp);
            let adjusted_lower = quantile(&adjusted_sorted, 0.025);
            let adjusted_upper = quantile(&adjusted_sorted, 0.975);
            let negative = prepared.negative_control[radius_index];
            let independent = prepared.independent_change[radius_index];
            let adjusted_excludes_zero = adjusted_lower > 0.0 || adjusted_upper < 0.0;
            let negative_compatible =
                negative >= deformation_lower && negative <= deformation_upper;
            let independent_agrees = (adjusted_mean > 0.0 && independent > 0.0)
                || (adjusted_mean < 0.0 && independent < 0.0);
            let values = [
                observed,
                deformation_mean,
                deformation_lower,
                deformation_upper,
                adjusted_mean,
                adjusted_lower,
                adjusted_upper,
                negative,
                independent,
            ];
            if values.into_iter().any(|value| !value.is_finite()) {
                return Err(RegisteredLongitudinalVoxelKError::Invalid(
                    "longitudinal change diagnostics are not finite".into(),
                ));
            }
            Ok(RegisteredLongitudinalCurvePoint {
                radius_um: baseline_row.radius_um,
                baseline_k_um3: baseline_row.k_um3,
                follow_up_k_um3: follow_up_row.k_um3,
                observed_delta_k_um3: observed,
                deformation_only_delta_mean_um3: deformation_mean,
                deformation_only_lower_95_um3: deformation_lower,
                deformation_only_upper_95_um3: deformation_upper,
                registration_adjusted_delta_mean_um3: adjusted_mean,
                registration_adjusted_lower_95_um3: adjusted_lower,
                registration_adjusted_upper_95_um3: adjusted_upper,
                negative_control_delta_k_um3: negative,
                independent_change_delta_k_um3: independent,
                adjusted_interval_excludes_zero: adjusted_excludes_zero,
                negative_control_compatible_with_registration: negative_compatible,
                independent_direction_agrees: independent_agrees,
                change_supported_beyond_registration: adjusted_excludes_zero
                    && negative_compatible
                    && independent_agrees,
            })
        })
        .collect()
}

fn quantile(sorted: &[f64], probability: f64) -> f64 {
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = position - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn assumptions() -> [&'static str; 4] {
    [
        "both_specimens_use_the_same_prespecified_k_estimand",
        "deformation_draws_are_registration_only_k_change_not_biological_replicates",
        "negative_control_and_independent_measurement_were_prespecified",
        "registered_proximity_does_not_establish_same_cell_correspondence",
    ]
}

fn validate_text(value: &str, field: &str) -> Result<(), RegisteredLongitudinalVoxelKError> {
    if value.is_empty() || value.trim() != value {
        return Err(RegisteredLongitudinalVoxelKError::Invalid(format!(
            "{field} must be nonempty without surrounding whitespace"
        )));
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> RegisteredLongitudinalVoxelKError {
    RegisteredLongitudinalVoxelKError::Invalid(error.to_string())
}
