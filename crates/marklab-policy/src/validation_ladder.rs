use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::PolicyError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationStageRecord {
    pub stage: u8,
    pub completed: bool,
    pub evidence_refs: Vec<String>,
    pub unresolved_risks: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationLadderSpec {
    pub method_id: String,
    pub stages: Vec<ValidationStageRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RealDataValidationLadderResult {
    pub format: &'static str,
    pub version: u32,
    pub method_id: String,
    pub highest_completed_stage: Option<u8>,
    pub highest_completed_label: Option<&'static str>,
    pub stages: Vec<ValidationStageRecord>,
    pub promotion_blocking_risks: Vec<String>,
    pub policy: &'static str,
}

pub fn evaluate_validation_ladder(
    spec: ValidationLadderSpec,
) -> Result<RealDataValidationLadderResult, PolicyError> {
    validate(&spec)?;
    let highest = spec
        .stages
        .iter()
        .take_while(|stage| stage.completed)
        .last()
        .map(|stage| stage.stage);
    let first_incomplete = highest.map_or(0, |stage| usize::from(stage) + 1);
    let mut seen_risks = HashSet::new();
    let promotion_blocking_risks = spec.stages[first_incomplete..]
        .iter()
        .flat_map(|stage| &stage.unresolved_risks)
        .filter_map(|risk| seen_risks.insert(risk.as_str()).then_some(risk.clone()))
        .collect();
    Ok(RealDataValidationLadderResult {
        format: "marklab.real_data_validation_ladder",
        version: 1,
        method_id: spec.method_id,
        highest_completed_stage: highest,
        highest_completed_label: highest.map(stage_label),
        stages: spec.stages,
        promotion_blocking_risks,
        policy: "part_xiii_validation_ladder_v1",
    })
}

fn validate(spec: &ValidationLadderSpec) -> Result<(), PolicyError> {
    if spec.method_id.trim().is_empty() || spec.stages.len() != 6 {
        return Err(PolicyError::Invalid(
            "method ID and exactly six validation stages are required".into(),
        ));
    }
    let mut seen_incomplete = false;
    let mut evidence = HashSet::new();
    for (expected, stage) in spec.stages.iter().enumerate() {
        if usize::from(stage.stage) != expected {
            return Err(PolicyError::Invalid(
                "validation stages must appear exactly once in order zero through five".into(),
            ));
        }
        if seen_incomplete && stage.completed {
            return Err(PolicyError::Invalid(
                "validation completion cannot resume after an incomplete stage".into(),
            ));
        }
        seen_incomplete |= !stage.completed;
        if stage.completed && stage.evidence_refs.is_empty() {
            return Err(PolicyError::Invalid(format!(
                "completed validation stage {} requires evidence",
                stage.stage
            )));
        }
        for reference in &stage.evidence_refs {
            if reference.trim().is_empty() || !evidence.insert(reference.as_str()) {
                return Err(PolicyError::Invalid(
                    "validation evidence references must be globally unique and nonempty".into(),
                ));
            }
        }
        let mut risks = HashSet::new();
        if stage
            .unresolved_risks
            .iter()
            .any(|risk| risk.trim().is_empty() || !risks.insert(risk.as_str()))
        {
            return Err(PolicyError::Invalid(
                "unresolved risks must be unique and nonempty within each stage".into(),
            ));
        }
    }
    Ok(())
}

const fn stage_label(stage: u8) -> &'static str {
    match stage {
        0 => "synthetic_truth_independent_oracle",
        1 => "public_technical_benchmark",
        2 => "internal_real_cohort_controls",
        3 => "heldout_same_site_patients",
        4 => "external_site_platform",
        5 => "prospective_or_perturbational_validation",
        _ => "invalid_stage",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_stage_after_gap_is_rejected() {
        let mut stages = (0..6)
            .map(|stage| ValidationStageRecord {
                stage,
                completed: stage < 2,
                evidence_refs: if stage < 2 {
                    vec![format!("e{stage}")]
                } else {
                    Vec::new()
                },
                unresolved_risks: Vec::new(),
            })
            .collect::<Vec<_>>();
        stages[3].completed = true;
        stages[3].evidence_refs = vec!["e3".into()];
        let error = evaluate_validation_ladder(ValidationLadderSpec {
            method_id: "method".into(),
            stages,
        })
        .unwrap_err();
        assert!(error.to_string().contains("cannot resume"));
    }
}
