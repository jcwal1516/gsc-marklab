use std::collections::BTreeMap;

use marklab_cohort::{
    max_t_multiple_endpoint_permutation, MaxTPermutationSpec, PatientEndpointVector,
};
use marklab_policy::{determine_result_maturity, ExecutionModeKind, Maturity, ResultMaturitySpec};

use crate::Result;

use super::{
    admission::PreparedStudy,
    invalid,
    model::{
        EndpointResult, InferenceResult, MaturityDescription, MultiplexStudyResult, PatientResult,
        SlideResult,
    },
};

pub(super) fn reduce(
    study: &PreparedStudy,
    slides: Vec<SlideResult>,
) -> Result<MultiplexStudyResult> {
    let (endpoint_names, patients) = patient_reduction(study, &slides)?;
    let inference = infer(study, &patients, &endpoint_names)?;
    let available = inference.status == "available";
    let decision = determine_result_maturity(ResultMaturitySpec {
        method_maturity: Maturity::Experimental,
        mode: ExecutionModeKind::Exact,
        provenance_complete: true,
        converged: true,
        severe_diagnostic_failure: !available,
        approximation_error_validated: false,
        predictive_clinical_claim: false,
        external_validation_complete: false,
        causal_claim: false,
        causal_identification_supported: false,
        bayesian: false,
        sbc_complete: false,
        posterior_predictive_complete: false,
    })
    .map_err(|error| invalid(error.to_string()))?;
    Ok(MultiplexStudyResult {
        format: "marklab.multiplex_study_result".into(), version: 1,
        study_id: study.recipe.study_id.clone(), recipe_sha256: study.recipe_digest.to_string(),
        channels: study.recipe.channels.clone(), design: study.recipe.design.clone(), limits: study.recipe.limits,
        endpoint_names, slides, patients, inference,
        maturity: MaturityDescription { format: decision.format.into(), version: decision.version,
            method_maturity: decision.method_maturity, result_maturity: decision.result_maturity,
            downgraded: decision.downgraded, reasons: decision.reasons.into_iter().map(str::to_owned).collect(), policy: decision.policy.into() },
        claim_scope: "Prespecified group differences in equally slide-averaged patient spatial summaries, conditional on observed per-channel cells and declared independent-patient exchangeability; no cell-level, causal or clinical claim.".into(),
        evidence_limits: vec![
            "Exact radius geometry and canonical Moran/Geary arithmetic; finite Monte Carlo whole-patient Max-T permutations, not exhaustive randomization.".into(),
            "Computational provenance binds the declared recipe and assay processing metadata; the original assay, biological identities and measurement status are not independently audited.".into(),
            "Missingness is per-channel complete-case selection, not a missing-at-random assertion or full-panel estimand.".into(),
            "Experimental workflow: bounded software/oracle coverage does not establish external biological calibration, clinical validity or whole-slide inference capacity.".into(),
            "Memory admission is a conservative retained-work estimate, not a measured peak-RSS guarantee.".into(),
        ],
    })
}

pub(super) fn patient_reduction(
    study: &PreparedStudy,
    slides: &[SlideResult],
) -> Result<(Vec<String>, Vec<PatientResult>)> {
    let endpoint_names = study
        .recipe
        .design
        .selected_channels
        .iter()
        .flat_map(|id| [format!("{id}:moran_i"), format!("{id}:geary_c")])
        .collect::<Vec<_>>();
    let mut by_patient = BTreeMap::<&str, Vec<&SlideResult>>::new();
    for slide in slides {
        by_patient.entry(&slide.patient_id).or_default().push(slide);
    }
    let mut patients = Vec::with_capacity(by_patient.len());
    for (id, patient_slides) in by_patient {
        let unavailable_slide_ids = patient_slides
            .iter()
            .filter(|slide| {
                slide
                    .channels
                    .iter()
                    .any(|channel| channel.status != "available")
            })
            .map(|slide| slide.slide_id.clone())
            .collect::<Vec<_>>();
        let values = if unavailable_slide_ids.is_empty() {
            let mut values = Vec::with_capacity(endpoint_names.len());
            for endpoint in 0..endpoint_names.len() {
                let per_slide = patient_slides
                    .iter()
                    .map(|slide| {
                        let channel = &slide.channels[endpoint / 2];
                        if endpoint % 2 == 0 {
                            channel.moran_i
                        } else {
                            channel.geary_c
                        }
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| invalid("available slide has no finite statistic"))?;
                values.push(equal_slide_mean(&per_slide)?);
            }
            Some(values)
        } else {
            None
        };
        patients.push(PatientResult {
            patient_id: id.into(),
            group: patient_slides[0].group.clone(),
            slide_count: patient_slides.len(),
            slide_ids: patient_slides
                .iter()
                .map(|slide| slide.slide_id.clone())
                .collect(),
            status: if values.is_some() {
                "available"
            } else {
                "unavailable"
            }
            .into(),
            values,
            unavailable_slide_ids,
        });
    }
    Ok((endpoint_names, patients))
}

fn infer(
    study: &PreparedStudy,
    patients: &[PatientResult],
    endpoints: &[String],
) -> Result<InferenceResult> {
    let unavailable = |reason: String, attempted| InferenceResult {
        status: "unavailable".into(),
        reason: Some(reason),
        correction: "single_step_max_t".into(),
        endpoints: Vec::new(),
        critical_value: None,
        permutations_attempted: attempted,
        permutations_completed: attempted.map(|_| 0),
    };
    if patients.iter().any(|patient| patient.values.is_none()) {
        return Ok(unavailable("required_slide_endpoint_unavailable; no slides, patients or endpoints were silently dropped".into(), Some(0)));
    }
    let vectors = patients
        .iter()
        .map(|patient| PatientEndpointVector {
            patient_id: patient.patient_id.clone(),
            group: patient.group.clone(),
            endpoints: endpoints.to_vec(),
            values: patient
                .values
                .clone()
                .expect("complete patient vectors checked"),
        })
        .collect::<Vec<_>>();
    let design = &study.recipe.design;
    match max_t_multiple_endpoint_permutation(
        &vectors,
        &MaxTPermutationSpec {
            group_a: design.group_a.clone(),
            group_b: design.group_b.clone(),
            permutations: design.permutations,
            seed: design.seed,
            alpha: design.alpha,
        },
    ) {
        Ok(result) => Ok(InferenceResult {
            status: "available".into(),
            reason: None,
            correction: result.correction.as_str().into(),
            endpoints: result
                .endpoints
                .into_iter()
                .map(|endpoint| EndpointResult {
                    endpoint: endpoint.endpoint,
                    effect_group_a_minus_group_b: endpoint.effect_group_a_minus_group_b,
                    studentized_statistic: endpoint.studentized_statistic,
                    adjusted_p_value: endpoint.adjusted_p_value,
                })
                .collect(),
            critical_value: Some(result.critical_value),
            permutations_attempted: Some(result.permutations_attempted),
            permutations_completed: Some(result.permutations_completed),
        }),
        // The canonical engine returns no partial replicate ledger on failure. Keep counts unknown
        // rather than inventing zero attempted fits or dropping a failed permutation.
        Err(error) => Ok(unavailable(format!("patient Max-T failed: {error}"), None)),
    }
}

fn equal_slide_mean(values: &[f64]) -> Result<f64> {
    let scale = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let mut total = 0.0;
    let mut correction = 0.0;
    if !scale.is_finite() || values.is_empty() {
        return Err(invalid("invalid patient slide summary"));
    }
    if scale == 0.0 {
        return Ok(0.0);
    }
    for value in values {
        let adjusted = value / scale - correction;
        let next = total + adjusted;
        correction = (next - total) - adjusted;
        total = next;
    }
    let mean = total / values.len() as f64 * scale;
    if !mean.is_finite() {
        return Err(invalid("patient slide mean is nonfinite"));
    }
    Ok(if mean == 0.0 { 0.0 } else { mean })
}
