use std::collections::HashSet;

use super::{welch_contrast, CohortInferenceError};

const MAXIMUM_CALIBRATION_ENDPOINTS: usize = 32;

/// One fixed complete endpoint vector used by the exhaustive calibration oracle.
#[derive(Clone, Debug)]
pub struct MaxTCalibrationRecord {
    /// Stable patient identifier.
    pub patient_id: String,
    /// Exact ordered endpoint names shared by every row.
    pub endpoints: Vec<String>,
    /// Finite values aligned to `endpoints`.
    pub values: Vec<f64>,
}

/// Bounded exhaustive whole-patient assignment calibration design.
#[derive(Clone, Debug)]
pub struct MaxTCalibrationSpec {
    /// Fixed number of rows assigned to group A in every assignment.
    pub group_a_count: usize,
    /// Contiguous ordered endpoint-family sizes; the sum must equal the endpoint count.
    pub family_sizes: Vec<usize>,
    /// Family-wise error rate evaluated exactly over all assignments.
    pub alpha: f64,
    /// Hard ceiling for the binomial number of group assignments.
    pub maximum_assignments: usize,
    /// Hard ceiling for assignment-by-assignment endpoint comparisons.
    pub maximum_assignment_endpoint_evaluations: u64,
    /// Hard retained-memory ceiling in bytes.
    pub memory_budget_bytes: usize,
}

/// Exact global-null calibration summary for one correction.
#[derive(Clone, Debug, PartialEq)]
pub struct MaxTCalibrationMethodResult {
    /// Assignments with at least one rejected endpoint.
    pub familywise_error_count: usize,
    /// Exact count divided by the complete assignment count.
    pub familywise_error_rate: f64,
    /// Per-endpoint exact rejection rates in source order.
    pub endpoint_rejection_rates: Vec<f64>,
    /// Whether the exact global-null rate is at most the declared alpha.
    pub calibration_status: String,
}

/// Work and retained-memory accounting for the exact calibration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaxTCalibrationWork {
    /// Complete assignment count.
    pub assignments: usize,
    /// Assignment-statistic patient-endpoint visits.
    pub statistic_patient_endpoint_evaluations: u64,
    /// Observed-assignment by null-assignment endpoint comparisons.
    pub assignment_endpoint_comparisons: u64,
    /// Conservative retained-memory accounting.
    pub retained_memory_bytes: usize,
}

/// Exact exhaustive calibration result for all currently owned Max-T corrections.
#[derive(Clone, Debug, PartialEq)]
pub struct MaxTCalibrationResult {
    /// Independent population unit.
    pub population_unit: String,
    /// Exact finite random-assignment null.
    pub null: String,
    /// Number of complete patient rows.
    pub patient_count: usize,
    /// Group-A row count in every assignment.
    pub group_a_count: usize,
    /// Complementary group-B row count.
    pub group_b_count: usize,
    /// Ordered endpoint names.
    pub endpoints: Vec<String>,
    /// Contiguous ordered family sizes.
    pub family_sizes: Vec<usize>,
    /// Complete number of fixed-size assignments.
    pub exact_assignment_count: usize,
    /// Declared family-wise alpha.
    pub alpha: f64,
    /// Single-step complete-family calibration.
    pub single_step: MaxTCalibrationMethodResult,
    /// Step-down complete-family calibration.
    pub step_down: MaxTCalibrationMethodResult,
    /// Ordered-family step-down gatekeeping calibration.
    pub ordered_gatekeeping_step_down: MaxTCalibrationMethodResult,
    /// Explicit bounded work and retained memory.
    pub work: MaxTCalibrationWork,
}

/// Exhaustively calibrate single-step, step-down, and ordered gatekeeping Max-T.
pub fn exact_max_t_calibration(
    records: &[MaxTCalibrationRecord],
    spec: &MaxTCalibrationSpec,
) -> Result<MaxTCalibrationResult, CohortInferenceError> {
    validate(records, spec)?;
    let assignment_count = binomial(records.len(), spec.group_a_count)?;
    if assignment_count > spec.maximum_assignments {
        return Err(invalid(format!(
            "exact assignment count {} exceeds maximum {}",
            assignment_count, spec.maximum_assignments
        )));
    }
    let endpoint_count = records[0].endpoints.len();
    let assignment_pairs = checked_product_u64(&[assignment_count, assignment_count])?;
    let endpoint_triangle = triangular(endpoint_count)?;
    let family_triangles = spec.family_sizes.iter().try_fold(0_u64, |sum, size| {
        sum.checked_add(triangular(*size)?)
            .ok_or_else(|| invalid("calibration family comparison accounting overflowed"))
    })?;
    let comparison_work = assignment_pairs
        .checked_mul(endpoint_count as u64 + endpoint_triangle + family_triangles)
        .and_then(|value| value.checked_add((assignment_count * endpoint_count) as u64))
        .ok_or_else(|| invalid("calibration comparison accounting overflowed"))?;
    if comparison_work > spec.maximum_assignment_endpoint_evaluations {
        return Err(invalid(format!(
            "assignment endpoint evaluations {comparison_work} exceed maximum {}",
            spec.maximum_assignment_endpoint_evaluations
        )));
    }
    let statistic_work = checked_product_u64(&[assignment_count, records.len(), endpoint_count])?;
    let retained_memory_bytes = retained_memory(records, assignment_count)?;
    if retained_memory_bytes > spec.memory_budget_bytes {
        return Err(invalid(format!(
            "calibration retained memory {retained_memory_bytes} exceeds budget {}",
            spec.memory_budget_bytes
        )));
    }
    let assignments = enumerate_assignments(records.len(), spec.group_a_count, assignment_count)?;

    let statistics = assignments
        .iter()
        .map(|labels| endpoint_statistics(records, labels))
        .collect::<Result<Vec<_>, _>>()?;
    let single_step = calibrate_single_step(&statistics, spec.alpha);
    let step_down = calibrate_step_down(&statistics, spec.alpha);
    let ordered_gatekeeping_step_down =
        calibrate_gatekeeping(&statistics, &spec.family_sizes, spec.alpha);
    Ok(MaxTCalibrationResult {
        population_unit: "whole_patient".into(),
        null: "every_fixed_size_whole_patient_group_assignment_is_equally_likely".into(),
        patient_count: records.len(),
        group_a_count: spec.group_a_count,
        group_b_count: records.len() - spec.group_a_count,
        endpoints: records[0].endpoints.clone(),
        family_sizes: spec.family_sizes.clone(),
        exact_assignment_count: assignments.len(),
        alpha: spec.alpha,
        single_step,
        step_down,
        ordered_gatekeeping_step_down,
        work: MaxTCalibrationWork {
            assignments: assignments.len(),
            statistic_patient_endpoint_evaluations: statistic_work,
            assignment_endpoint_comparisons: comparison_work,
            retained_memory_bytes,
        },
    })
}

fn validate(
    records: &[MaxTCalibrationRecord],
    spec: &MaxTCalibrationSpec,
) -> Result<(), CohortInferenceError> {
    if records.len() < 4 {
        return Err(invalid("Max-T calibration requires at least four patients"));
    }
    if spec.group_a_count < 2 || records.len().saturating_sub(spec.group_a_count) < 2 {
        return Err(invalid(
            "Max-T calibration requires at least two patients in each group",
        ));
    }
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 1.0) {
        return Err(invalid(
            "Max-T calibration alpha must be finite and strictly between zero and one",
        ));
    }
    if spec.maximum_assignments == 0
        || spec.maximum_assignment_endpoint_evaluations == 0
        || spec.memory_budget_bytes == 0
    {
        return Err(invalid("Max-T calibration limits must be positive"));
    }
    let endpoints = &records[0].endpoints;
    if endpoints.is_empty() || endpoints.len() > MAXIMUM_CALIBRATION_ENDPOINTS {
        return Err(invalid(format!(
            "Max-T calibration requires 1 to {MAXIMUM_CALIBRATION_ENDPOINTS} endpoints"
        )));
    }
    if endpoints.iter().any(|value| value.trim().is_empty())
        || endpoints.iter().collect::<HashSet<_>>().len() != endpoints.len()
    {
        return Err(invalid(
            "Max-T calibration endpoint names must be nonempty and unique",
        ));
    }
    let family_endpoint_count = spec
        .family_sizes
        .iter()
        .try_fold(0usize, |sum, size| sum.checked_add(*size));
    if spec.family_sizes.is_empty()
        || spec.family_sizes.contains(&0)
        || family_endpoint_count != Some(endpoints.len())
    {
        return Err(invalid(
            "ordered family sizes must be positive and partition every endpoint",
        ));
    }
    let mut patient_ids = HashSet::with_capacity(records.len());
    for record in records {
        if record.patient_id.trim().is_empty()
            || record.patient_id.trim() != record.patient_id
            || !patient_ids.insert(record.patient_id.as_str())
        {
            return Err(invalid(
                "calibration patient IDs must be exact, nonempty, and unique",
            ));
        }
        if record.endpoints != *endpoints
            || record.values.len() != endpoints.len()
            || record.values.iter().any(|value| !value.is_finite())
        {
            return Err(invalid(
                "every calibration row must carry the same complete finite endpoint vector",
            ));
        }
    }
    Ok(())
}

fn enumerate_assignments(
    patient_count: usize,
    group_a_count: usize,
    expected_count: usize,
) -> Result<Vec<Vec<bool>>, CohortInferenceError> {
    let mut indices = (0..group_a_count).collect::<Vec<_>>();
    let mut assignments = Vec::new();
    assignments
        .try_reserve_exact(expected_count)
        .map_err(|_| invalid("calibration assignment allocation failed"))?;
    loop {
        let mut labels = vec![false; patient_count];
        for index in &indices {
            labels[*index] = true;
        }
        assignments.push(labels);
        let Some(position) = (0..group_a_count)
            .rev()
            .find(|position| indices[*position] < patient_count - group_a_count + *position)
        else {
            break;
        };
        indices[position] += 1;
        for next in position + 1..group_a_count {
            indices[next] = indices[next - 1] + 1;
        }
    }
    if assignments.len() != expected_count {
        return Err(invalid("calibration assignment enumeration count differs"));
    }
    Ok(assignments)
}

fn endpoint_statistics(
    records: &[MaxTCalibrationRecord],
    labels: &[bool],
) -> Result<Vec<f64>, CohortInferenceError> {
    (0..records[0].values.len())
        .map(|endpoint| {
            let values = records
                .iter()
                .map(|record| record.values[endpoint])
                .collect::<Vec<_>>();
            welch_contrast(&values, labels).map(|contrast| contrast.studentized)
        })
        .collect()
}

fn calibrate_single_step(statistics: &[Vec<f64>], alpha: f64) -> MaxTCalibrationMethodResult {
    let maxima = statistics
        .iter()
        .map(|row| row.iter().map(|value| value.abs()).fold(0.0_f64, f64::max))
        .collect::<Vec<_>>();
    calibrate(statistics, alpha, |observed| {
        observed
            .iter()
            .map(|value| {
                maxima
                    .iter()
                    .filter(|maximum| **maximum >= value.abs())
                    .count() as f64
                    / statistics.len() as f64
            })
            .collect()
    })
}

fn calibrate_step_down(statistics: &[Vec<f64>], alpha: f64) -> MaxTCalibrationMethodResult {
    calibrate(statistics, alpha, |observed| {
        exact_step_down_p_values(observed, statistics)
    })
}

fn calibrate_gatekeeping(
    statistics: &[Vec<f64>],
    family_sizes: &[usize],
    alpha: f64,
) -> MaxTCalibrationMethodResult {
    let endpoint_count = statistics[0].len();
    let mut familywise_error_count = 0usize;
    let mut endpoint_rejections = vec![0usize; endpoint_count];
    for observed_index in 0..statistics.len() {
        let mut offset = 0usize;
        let mut opened = true;
        let mut rejected_any = false;
        for family_size in family_sizes {
            let end = offset + family_size;
            let local_null = statistics
                .iter()
                .map(|row| row[offset..end].to_vec())
                .collect::<Vec<_>>();
            let p_values =
                exact_step_down_p_values(&statistics[observed_index][offset..end], &local_null);
            if opened {
                let all_rejected = p_values.iter().all(|value| *value <= alpha);
                for (local, value) in p_values.iter().enumerate() {
                    if *value <= alpha {
                        endpoint_rejections[offset + local] += 1;
                        rejected_any = true;
                    }
                }
                opened = all_rejected;
            }
            offset = end;
        }
        familywise_error_count += usize::from(rejected_any);
    }
    method_result(
        familywise_error_count,
        endpoint_rejections,
        statistics.len(),
        alpha,
    )
}

fn calibrate<F>(statistics: &[Vec<f64>], alpha: f64, p_values: F) -> MaxTCalibrationMethodResult
where
    F: Fn(&[f64]) -> Vec<f64>,
{
    let mut familywise_error_count = 0usize;
    let mut endpoint_rejections = vec![0usize; statistics[0].len()];
    for observed in statistics {
        let rejected = p_values(observed)
            .into_iter()
            .map(|value| value <= alpha)
            .collect::<Vec<_>>();
        familywise_error_count += usize::from(rejected.iter().any(|value| *value));
        for (count, rejected) in endpoint_rejections.iter_mut().zip(rejected) {
            *count += usize::from(rejected);
        }
    }
    method_result(
        familywise_error_count,
        endpoint_rejections,
        statistics.len(),
        alpha,
    )
}

fn exact_step_down_p_values(observed: &[f64], null: &[Vec<f64>]) -> Vec<f64> {
    let mut order = (0..observed.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        observed[*right]
            .abs()
            .total_cmp(&observed[*left].abs())
            .then_with(|| left.cmp(right))
    });
    let mut p_values = vec![0.0; observed.len()];
    let mut previous = 0.0_f64;
    for rank in 0..order.len() {
        let endpoint = order[rank];
        let exceedances = null
            .iter()
            .filter(|row| {
                order[rank..]
                    .iter()
                    .map(|index| row[*index].abs())
                    .fold(0.0_f64, f64::max)
                    >= observed[endpoint].abs()
            })
            .count();
        previous = previous.max(exceedances as f64 / null.len() as f64);
        p_values[endpoint] = previous;
    }
    p_values
}

fn method_result(
    familywise_error_count: usize,
    endpoint_rejections: Vec<usize>,
    assignment_count: usize,
    alpha: f64,
) -> MaxTCalibrationMethodResult {
    let denominator = assignment_count as f64;
    let familywise_error_rate = familywise_error_count as f64 / denominator;
    MaxTCalibrationMethodResult {
        familywise_error_count,
        familywise_error_rate,
        endpoint_rejection_rates: endpoint_rejections
            .into_iter()
            .map(|count| count as f64 / denominator)
            .collect(),
        calibration_status: if familywise_error_rate <= alpha {
            "controlled_at_declared_alpha"
        } else {
            "exceeds_declared_alpha"
        }
        .into(),
    }
}

fn retained_memory(
    records: &[MaxTCalibrationRecord],
    assignment_count: usize,
) -> Result<usize, CohortInferenceError> {
    let endpoint_count = records[0].endpoints.len();
    let source = records.iter().try_fold(0usize, |total, record| {
        total
            .checked_add(record.patient_id.len())
            .and_then(|value| {
                value.checked_add(record.endpoints.iter().map(String::len).sum::<usize>())
            })
            .and_then(|value| value.checked_add(record.values.len() * std::mem::size_of::<f64>()))
    });
    source
        .and_then(|value| value.checked_add(assignment_count * records.len()))
        .and_then(|value| {
            value.checked_add(assignment_count * endpoint_count * std::mem::size_of::<f64>())
        })
        .and_then(|value| {
            value.checked_add(
                assignment_count * records[0].endpoints.len() * std::mem::size_of::<f64>(),
            )
        })
        .and_then(|value| value.checked_add(endpoint_count * 8 * std::mem::size_of::<f64>()))
        .ok_or_else(|| invalid("calibration retained-memory accounting overflowed"))
}

fn triangular(value: usize) -> Result<u64, CohortInferenceError> {
    let value = value as u64;
    value
        .checked_mul(value + 1)
        .map(|product| product / 2)
        .ok_or_else(|| invalid("calibration triangular work accounting overflowed"))
}

fn binomial(patient_count: usize, group_a_count: usize) -> Result<usize, CohortInferenceError> {
    let selected = group_a_count.min(patient_count - group_a_count);
    let mut value = 1_u128;
    for index in 1..=selected {
        value = value
            .checked_mul((patient_count - selected + index) as u128)
            .ok_or_else(|| invalid("exact assignment count overflowed"))?
            / index as u128;
        if value > usize::MAX as u128 {
            return Err(invalid("exact assignment count exceeds platform capacity"));
        }
    }
    Ok(value as usize)
}

fn checked_product_u64(values: &[usize]) -> Result<u64, CohortInferenceError> {
    values.iter().try_fold(1u64, |product, value| {
        product
            .checked_mul(*value as u64)
            .ok_or_else(|| invalid("calibration work accounting overflowed"))
    })
}

fn invalid(message: impl Into<String>) -> CohortInferenceError {
    CohortInferenceError::InvalidInput(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<MaxTCalibrationRecord> {
        let values = [
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 4.0],
            [2.0, 1.0, 3.0],
            [3.0, 4.0, 1.0],
            [4.0, 3.0, 7.0],
            [5.0, 7.0, 2.0],
            [6.0, 5.0, 8.0],
            [7.0, 6.0, 5.0],
        ];
        values
            .into_iter()
            .enumerate()
            .map(|(index, values)| MaxTCalibrationRecord {
                patient_id: format!("p{}", index + 1),
                endpoints: vec!["e1".into(), "e2".into(), "e3".into()],
                values: values.to_vec(),
            })
            .collect()
    }

    #[test]
    fn exact_assignment_oracle_controls_all_three_corrections() {
        let result = exact_max_t_calibration(
            &fixture(),
            &MaxTCalibrationSpec {
                group_a_count: 4,
                family_sizes: vec![1, 2],
                alpha: 0.1,
                maximum_assignments: 70,
                maximum_assignment_endpoint_evaluations: 63_910,
                memory_budget_bytes: 1024 * 1024,
            },
        )
        .expect("calibration");
        assert_eq!(result.exact_assignment_count, 70);
        assert_eq!(result.work.assignment_endpoint_comparisons, 63_910);
        assert_eq!(result.single_step.familywise_error_count, 6);
        assert_eq!(result.step_down.familywise_error_count, 6);
        assert_eq!(
            result.ordered_gatekeeping_step_down.familywise_error_count,
            4
        );
        assert!(result.single_step.familywise_error_rate <= 0.1);
        assert!(result.step_down.familywise_error_rate <= 0.1);
        assert!(result.ordered_gatekeeping_step_down.familywise_error_rate <= 0.1);
    }

    #[test]
    fn one_short_comparison_limit_is_rejected() {
        let error = exact_max_t_calibration(
            &fixture(),
            &MaxTCalibrationSpec {
                group_a_count: 4,
                family_sizes: vec![1, 2],
                alpha: 0.1,
                maximum_assignments: 70,
                maximum_assignment_endpoint_evaluations: 63_909,
                memory_budget_bytes: 1024 * 1024,
            },
        )
        .expect_err("one-short work must fail");
        assert!(error.to_string().contains("63910"));
    }
}
