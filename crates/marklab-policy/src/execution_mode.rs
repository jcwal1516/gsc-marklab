use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{ExecutionModeKind, PolicyError};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionModeDescriptor {
    pub mode_id: String,
    pub kind: ExecutionModeKind,
    pub required_backend: String,
    pub memory_base_bytes: u64,
    pub memory_per_item_bytes: u64,
    pub runtime_base_units: u64,
    pub runtime_per_item_units: u64,
    pub validated_max_error: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionModeSelectionSpec {
    pub available_backends: Vec<String>,
    pub data_size: u64,
    pub memory_budget_bytes: u64,
    pub runtime_budget_units: u64,
    pub requested_mode: Option<String>,
    pub approximation_approved: bool,
    pub requested_max_error: Option<f64>,
    pub modes: Vec<ExecutionModeDescriptor>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutionModeAssessment {
    pub mode_id: String,
    pub kind: ExecutionModeKind,
    pub required_backend: String,
    pub backend_available: bool,
    pub estimated_memory_bytes: u64,
    pub estimated_runtime_units: u64,
    pub memory_within_budget: bool,
    pub runtime_within_budget: bool,
    pub accuracy_within_request: bool,
    pub approximation_approved: bool,
    pub feasible_and_permitted: bool,
    pub rejection_reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionModeSelectionStatus {
    Selected,
    RequestedModeUnsupported,
    BackendUnavailable,
    ResourceLimitExceeded,
    AccuracyNotMet,
    ApproximationNotPermitted,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutionModeSelection {
    pub format: &'static str,
    pub version: u32,
    pub status: ExecutionModeSelectionStatus,
    pub requested_mode: Option<String>,
    pub selected_mode: Option<String>,
    pub selected_kind: Option<ExecutionModeKind>,
    pub approval_required_mode: Option<String>,
    pub assessments: Vec<ExecutionModeAssessment>,
    pub policy: &'static str,
}

pub fn select_execution_mode(
    spec: ExecutionModeSelectionSpec,
) -> Result<ExecutionModeSelection, PolicyError> {
    validate(&spec)?;
    let available = spec
        .available_backends
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let assessments = spec
        .modes
        .iter()
        .map(|mode| assess(mode, &spec, &available))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(requested) = &spec.requested_mode {
        let Some(index) = spec
            .modes
            .iter()
            .position(|mode| &mode.mode_id == requested)
        else {
            return Ok(decision(
                ExecutionModeSelectionStatus::RequestedModeUnsupported,
                &spec,
                None,
                None,
                assessments,
            ));
        };
        let assessment = &assessments[index];
        if assessment.feasible_and_permitted {
            return Ok(decision(
                ExecutionModeSelectionStatus::Selected,
                &spec,
                Some(index),
                None,
                assessments,
            ));
        }
        let status = status_for_reason(assessment.rejection_reason);
        let approval = matches!(
            status,
            ExecutionModeSelectionStatus::ApproximationNotPermitted
        )
        .then_some(index);
        return Ok(decision(status, &spec, None, approval, assessments));
    }
    if let Some(index) = assessments
        .iter()
        .position(|assessment| assessment.feasible_and_permitted)
    {
        return Ok(decision(
            ExecutionModeSelectionStatus::Selected,
            &spec,
            Some(index),
            None,
            assessments,
        ));
    }
    if let Some(index) = assessments
        .iter()
        .position(|assessment| assessment.rejection_reason == Some("approximation_not_approved"))
    {
        return Ok(decision(
            ExecutionModeSelectionStatus::ApproximationNotPermitted,
            &spec,
            None,
            Some(index),
            assessments,
        ));
    }
    Ok(decision(
        ExecutionModeSelectionStatus::ResourceLimitExceeded,
        &spec,
        None,
        None,
        assessments,
    ))
}

fn assess(
    mode: &ExecutionModeDescriptor,
    spec: &ExecutionModeSelectionSpec,
    available: &HashSet<&str>,
) -> Result<ExecutionModeAssessment, PolicyError> {
    let memory = mode
        .memory_per_item_bytes
        .checked_mul(spec.data_size)
        .and_then(|value| value.checked_add(mode.memory_base_bytes))
        .ok_or_else(|| {
            PolicyError::Invalid(format!("mode {} memory estimate overflowed", mode.mode_id))
        })?;
    let runtime = mode
        .runtime_per_item_units
        .checked_mul(spec.data_size)
        .and_then(|value| value.checked_add(mode.runtime_base_units))
        .ok_or_else(|| {
            PolicyError::Invalid(format!("mode {} runtime estimate overflowed", mode.mode_id))
        })?;
    let backend_available = available.contains(mode.required_backend.as_str());
    let memory_within_budget = memory <= spec.memory_budget_bytes;
    let runtime_within_budget = runtime <= spec.runtime_budget_units;
    let mode_error = match mode.kind {
        ExecutionModeKind::Exact => 0.0,
        ExecutionModeKind::Approximate => mode
            .validated_max_error
            .expect("validated approximate error required"),
    };
    let accuracy_within_request = spec
        .requested_max_error
        .is_none_or(|requested| mode_error <= requested);
    let approximation_approved =
        mode.kind == ExecutionModeKind::Exact || spec.approximation_approved;
    let rejection_reason = if !backend_available {
        Some("backend_unavailable")
    } else if !memory_within_budget {
        Some("memory_budget_exceeded")
    } else if !runtime_within_budget {
        Some("runtime_budget_exceeded")
    } else if !accuracy_within_request {
        Some("requested_accuracy_not_met")
    } else if !approximation_approved {
        Some("approximation_not_approved")
    } else {
        None
    };
    Ok(ExecutionModeAssessment {
        mode_id: mode.mode_id.clone(),
        kind: mode.kind,
        required_backend: mode.required_backend.clone(),
        backend_available,
        estimated_memory_bytes: memory,
        estimated_runtime_units: runtime,
        memory_within_budget,
        runtime_within_budget,
        accuracy_within_request,
        approximation_approved,
        feasible_and_permitted: rejection_reason.is_none(),
        rejection_reason,
    })
}

fn validate(spec: &ExecutionModeSelectionSpec) -> Result<(), PolicyError> {
    if spec.data_size == 0
        || spec.memory_budget_bytes == 0
        || spec.runtime_budget_units == 0
        || spec.modes.is_empty()
        || spec
            .requested_max_error
            .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(PolicyError::Invalid(
            "data/resource/mode/error requirements are invalid".into(),
        ));
    }
    let mut backends = HashSet::new();
    if spec
        .available_backends
        .iter()
        .any(|backend| backend.trim().is_empty() || !backends.insert(backend.as_str()))
    {
        return Err(PolicyError::Invalid(
            "available backend IDs must be unique and nonempty".into(),
        ));
    }
    let mut modes = HashSet::new();
    for mode in &spec.modes {
        let error_valid = match mode.kind {
            ExecutionModeKind::Exact => mode.validated_max_error.is_none(),
            ExecutionModeKind::Approximate => mode
                .validated_max_error
                .is_some_and(|value| value.is_finite() && value >= 0.0),
        };
        if mode.mode_id.trim().is_empty()
            || mode.required_backend.trim().is_empty()
            || !modes.insert(mode.mode_id.as_str())
            || !error_valid
        {
            return Err(PolicyError::Invalid(
                "mode IDs/backends must be nonempty and unique with kind-correct error bounds"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn decision(
    status: ExecutionModeSelectionStatus,
    spec: &ExecutionModeSelectionSpec,
    selected: Option<usize>,
    approval: Option<usize>,
    assessments: Vec<ExecutionModeAssessment>,
) -> ExecutionModeSelection {
    ExecutionModeSelection {
        format: "marklab.execution_mode_selection",
        version: 1,
        status,
        requested_mode: spec.requested_mode.clone(),
        selected_mode: selected.map(|index| spec.modes[index].mode_id.clone()),
        selected_kind: selected.map(|index| spec.modes[index].kind),
        approval_required_mode: approval.map(|index| spec.modes[index].mode_id.clone()),
        assessments,
        policy: "part_xiii_execution_mode_v1",
    }
}

fn status_for_reason(reason: Option<&str>) -> ExecutionModeSelectionStatus {
    match reason {
        Some("backend_unavailable") => ExecutionModeSelectionStatus::BackendUnavailable,
        Some("requested_accuracy_not_met") => ExecutionModeSelectionStatus::AccuracyNotMet,
        Some("approximation_not_approved") => {
            ExecutionModeSelectionStatus::ApproximationNotPermitted
        }
        _ => ExecutionModeSelectionStatus::ResourceLimitExceeded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_selection_can_skip_unapproved_approximation_for_later_exact_mode() {
        let result = select_execution_mode(ExecutionModeSelectionSpec {
            available_backends: vec!["cpu".into()],
            data_size: 1,
            memory_budget_bytes: 10,
            runtime_budget_units: 10,
            requested_mode: None,
            approximation_approved: false,
            requested_max_error: None,
            modes: vec![
                mode("approx", ExecutionModeKind::Approximate, Some(0.1)),
                mode("exact", ExecutionModeKind::Exact, None),
            ],
        })
        .unwrap();
        assert_eq!(result.selected_mode.as_deref(), Some("exact"));
    }

    fn mode(
        mode_id: &str,
        kind: ExecutionModeKind,
        validated_max_error: Option<f64>,
    ) -> ExecutionModeDescriptor {
        ExecutionModeDescriptor {
            mode_id: mode_id.into(),
            kind,
            required_backend: "cpu".into(),
            memory_base_bytes: 1,
            memory_per_item_bytes: 1,
            runtime_base_units: 1,
            runtime_per_item_units: 1,
            validated_max_error,
        }
    }
}
