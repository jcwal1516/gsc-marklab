use marklab_cohort::{
    patient_blocked_covariate_matrix_freedman_lane, patient_covariate_matrix_freedman_lane,
    CovariateMatrixPatientRecord, CovariatePermutationSpec, PatientExchangeabilityBlock,
    PermutationAlternative,
};

const NAMESPACE: u64 = 0x636f_765f_6d61_7478;

#[test]
fn covariate_matrix_unblocked_and_blocked_results_match_a_qr_fwl_oracle() {
    let records = records();
    let spec = spec();
    let all = vec![(0..records.len()).collect::<Vec<_>>()];
    let unblocked =
        patient_covariate_matrix_freedman_lane(&records, &spec).expect("unblocked result");
    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(
        unblocked,
        patient_covariate_matrix_freedman_lane(&reversed, &spec).expect("reordered result")
    );
    let mut rescaled = records.clone();
    for record in &mut rescaled {
        record.covariates[0] *= 1e100;
        record.covariates[1] *= 1e-100;
    }
    let rescaled_result =
        patient_covariate_matrix_freedman_lane(&rescaled, &spec).expect("rescaled result");
    assert!(
        (unblocked.effect_group_a_minus_group_b - rescaled_result.effect_group_a_minus_group_b)
            .abs()
            < 1e-12
    );
    assert_eq!(unblocked.p_value, rescaled_result.p_value);
    let reference = slow_reference(&records, &spec, &all);
    assert!((unblocked.effect_group_a_minus_group_b - reference.0).abs() < 1e-12);
    assert!((unblocked.target_standard_error - reference.1).abs() < 1e-12);
    assert!((unblocked.studentized_statistic - reference.2).abs() < 1e-12);
    assert_eq!(unblocked.p_value, reference.3);

    let mut assignments = records
        .iter()
        .map(|record| {
            let number = record.patient_id[2..].parse::<usize>().expect("number");
            PatientExchangeabilityBlock::new(
                record.patient_id.clone(),
                if number <= 3 { "north" } else { "south" },
            )
            .expect("assignment")
        })
        .collect::<Vec<_>>();
    assignments.reverse();
    let blocked = patient_blocked_covariate_matrix_freedman_lane(&records, &assignments, &spec)
        .expect("blocked result");
    let blocks = [vec![0, 1, 2, 6, 7, 8], vec![3, 4, 5, 9, 10, 11]];
    assert_eq!(blocked.p_value, slow_reference(&records, &spec, &blocks).3);
    assert!(blocked.blocked);
    assert_eq!(blocked.block_count, 2);
}

#[test]
fn covariate_matrix_rejects_missing_and_collinear_columns() {
    let mut missing = records();
    missing[0].covariate_names.pop();
    missing[0].covariates.pop();
    assert!(patient_covariate_matrix_freedman_lane(&missing, &spec()).is_err());

    let mut collinear = records();
    for record in &mut collinear {
        record.covariates[1] = 2.0 * record.covariates[0];
    }
    assert!(matches!(
        patient_covariate_matrix_freedman_lane(&collinear, &spec()),
        Err(error) if error.to_string().contains("rank deficient")
    ));
}

fn records() -> Vec<CovariateMatrixPatientRecord> {
    let noise = [-1.0, 0.5, -1.0, 1.0, -0.5, 1.0];
    ["A", "B"]
        .into_iter()
        .flat_map(|group| {
            noise.into_iter().enumerate().map(move |(index, noise)| {
                let batch = (index % 2) as f64;
                CovariateMatrixPatientRecord {
                    patient_id: format!("{}-{}", group.to_ascii_lowercase(), index + 1),
                    group: group.into(),
                    outcome: 2.0 * index as f64
                        + 0.5 * batch
                        + noise
                        + if group == "A" { 3.0 } else { 0.0 },
                    covariate_names: vec!["age".into(), "batch".into()],
                    covariates: vec![index as f64, batch],
                }
            })
        })
        .collect()
}

fn spec() -> CovariatePermutationSpec {
    CovariatePermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 53,
        alternative: PermutationAlternative::TwoSided,
    }
}

fn slow_reference(
    records: &[CovariateMatrixPatientRecord],
    spec: &CovariatePermutationSpec,
    blocks: &[Vec<usize>],
) -> (f64, f64, f64, f64) {
    let nuisance = records
        .iter()
        .map(|record| vec![1.0, record.covariates[0], record.covariates[1]])
        .collect::<Vec<_>>();
    let group = records
        .iter()
        .map(|record| f64::from(record.group == spec.group_a))
        .collect::<Vec<_>>();
    let outcome = records
        .iter()
        .map(|record| record.outcome)
        .collect::<Vec<_>>();
    let fitted = qr_fitted(&nuisance, &outcome);
    let residuals = outcome
        .iter()
        .zip(&fitted)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let observed = fwl(&nuisance, &group, &outcome);
    let mut lower = 0usize;
    let mut upper = 0usize;
    for replicate in 0..spec.permutations {
        let mut indices = (0..records.len()).collect::<Vec<_>>();
        let mut state = derive_seed(spec.seed, replicate);
        for block in blocks {
            let mut shuffled = block.clone();
            for index in (1..shuffled.len()).rev() {
                state = splitmix64(state ^ index as u64);
                let other = (state % (index as u64 + 1)) as usize;
                shuffled.swap(index, other);
            }
            for (position, source) in block.iter().zip(shuffled) {
                indices[*position] = source;
            }
        }
        let permuted = fitted
            .iter()
            .enumerate()
            .map(|(index, fit)| fit + residuals[indices[index]])
            .collect::<Vec<_>>();
        let statistic = fwl(&nuisance, &group, &permuted).2;
        lower += usize::from(statistic <= observed.2);
        upper += usize::from(statistic >= observed.2);
    }
    let p = (2.0 * ((lower.min(upper) as f64 + 1.0) / (spec.permutations + 1) as f64)).min(1.0);
    (observed.0, observed.1, observed.2, p)
}

fn fwl(nuisance: &[Vec<f64>], group: &[f64], outcome: &[f64]) -> (f64, f64, f64) {
    let group_fit = qr_fitted(nuisance, group);
    let outcome_fit = qr_fitted(nuisance, outcome);
    let group_residual = group
        .iter()
        .zip(group_fit)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let outcome_residual = outcome
        .iter()
        .zip(outcome_fit)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let denominator = dot(&group_residual, &group_residual);
    let coefficient = dot(&group_residual, &outcome_residual) / denominator;
    let rss = outcome_residual
        .iter()
        .zip(&group_residual)
        .map(|(outcome, group)| (outcome - coefficient * group).powi(2))
        .sum::<f64>();
    let degrees = outcome.len() - nuisance[0].len() - 1;
    let standard_error = (rss / degrees as f64 / denominator).sqrt();
    (coefficient, standard_error, coefficient / standard_error)
}

fn qr_fitted(design: &[Vec<f64>], outcome: &[f64]) -> Vec<f64> {
    let columns = design[0].len();
    let mut q = Vec::<Vec<f64>>::with_capacity(columns);
    let mut r = vec![vec![0.0; columns]; columns];
    for column in 0..columns {
        let mut vector = design.iter().map(|row| row[column]).collect::<Vec<_>>();
        for prior in 0..column {
            r[prior][column] = dot(&q[prior], &vector);
            for (value, basis) in vector.iter_mut().zip(&q[prior]) {
                *value -= r[prior][column] * basis;
            }
        }
        r[column][column] = dot(&vector, &vector).sqrt();
        assert!(r[column][column] > 1e-12, "oracle design rank");
        for value in &mut vector {
            *value /= r[column][column];
        }
        q.push(vector);
    }
    let qty = q
        .iter()
        .map(|column| dot(column, outcome))
        .collect::<Vec<_>>();
    let mut beta = vec![0.0; columns];
    for row in (0..columns).rev() {
        let remainder = ((row + 1)..columns)
            .map(|column| r[row][column] * beta[column])
            .sum::<f64>();
        beta[row] = (qty[row] - remainder) / r[row][row];
    }
    design.iter().map(|row| dot(row, &beta)).collect::<Vec<_>>()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn derive_seed(base_seed: u64, replicate: usize) -> u64 {
    splitmix64(splitmix64(base_seed ^ NAMESPACE) ^ replicate as u64)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}
