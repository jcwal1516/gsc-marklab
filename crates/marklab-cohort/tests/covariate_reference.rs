use marklab_cohort::{
    patient_blocked_covariate_freedman_lane, patient_covariate_freedman_lane,
    CovariatePatientRecord, CovariatePermutationSpec, InferenceAlternative, InferenceNullFamily,
    InferencePermutationUnit, PatientExchangeabilityBlock, PermutationAlternative,
};

const NAMESPACE: u64 = 0x636f_765f_666c_706d;

#[test]
fn patient_covariate_freedman_lane_matches_an_independent_fwl_reference() {
    let records = records();
    let spec = CovariatePermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 47,
        alternative: PermutationAlternative::TwoSided,
    };
    let result = patient_covariate_freedman_lane(&records, &spec).expect("result");
    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(
        result,
        patient_covariate_freedman_lane(&reversed, &spec).expect("reordered result")
    );
    let mut rescaled = records.clone();
    for record in &mut rescaled {
        record.covariate *= 1e100;
    }
    let rescaled_result =
        patient_covariate_freedman_lane(&rescaled, &spec).expect("rescaled covariate result");
    assert!(
        (result.effect_group_a_minus_group_b - rescaled_result.effect_group_a_minus_group_b).abs()
            < 1e-12
    );
    assert!((result.studentized_statistic - rescaled_result.studentized_statistic).abs() < 1e-12);
    assert_eq!(result.p_value, rescaled_result.p_value);
    let reference = slow_reference(&records, &spec, &[(0..records.len()).collect::<Vec<_>>()]);
    assert!((result.effect_group_a_minus_group_b - reference.0).abs() < 1e-12);
    assert!((result.target_standard_error - reference.1).abs() < 1e-12);
    assert!((result.studentized_statistic - reference.2).abs() < 1e-12);
    assert_eq!(result.p_value, reference.3);
    assert_eq!(
        result.inference_design.null_family(),
        InferenceNullFamily::CovariateConditionalResidualPermutation
    );
    assert_eq!(
        result.inference_design.permutation_unit(),
        InferencePermutationUnit::CompletePatientResidual
    );
    assert_eq!(
        result.inference_design.alternative(),
        InferenceAlternative::TwoSided
    );
}

#[test]
fn blocked_patient_covariate_freedman_lane_matches_restricted_residual_oracle() {
    let records = records();
    let spec = CovariatePermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 47,
        alternative: PermutationAlternative::TwoSided,
    };
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
    let result = patient_blocked_covariate_freedman_lane(&records, &assignments, &spec)
        .expect("blocked result");
    let blocks = [vec![0, 1, 2, 6, 7, 8], vec![3, 4, 5, 9, 10, 11]];
    let reference = slow_reference(&records, &spec, &blocks);
    assert!((result.effect_group_a_minus_group_b - reference.0).abs() < 1e-12);
    assert!((result.target_standard_error - reference.1).abs() < 1e-12);
    assert!((result.studentized_statistic - reference.2).abs() < 1e-12);
    assert_eq!(result.p_value, reference.3);
    assert!(result.blocked);
    assert_eq!(result.block_count, 2);
    assert_eq!(result.reduced_model_columns, 3);
    assert_eq!(result.residual_degrees_of_freedom, 8);
}

#[test]
fn blocked_covariate_effect_is_the_within_block_contrast() {
    // Group A is concentrated in the high-outcome block, but within each block the groups do not differ.
    let fixture = [
        ("north", "A", 1.0, 11.5),
        ("north", "A", 2.0, 10.0),
        ("north", "A", 3.0, 12.0),
        ("north", "A", 4.0, 11.5),
        ("north", "B", 2.0, 12.0),
        ("north", "B", 3.0, 10.5),
        ("south", "A", 1.0, 1.0),
        ("south", "A", 4.0, 1.5),
        ("south", "B", 1.0, 1.5),
        ("south", "B", 2.0, 0.0),
        ("south", "B", 3.0, 2.0),
        ("south", "B", 4.0, 1.5),
    ];
    let records = fixture
        .iter()
        .enumerate()
        .map(|(index, (_, group, age, outcome))| CovariatePatientRecord {
            patient_id: format!("p-{index:02}"),
            group: (*group).into(),
            outcome: *outcome,
            covariate: *age,
        })
        .collect::<Vec<_>>();
    let assignments = fixture
        .iter()
        .enumerate()
        .map(|(index, (block, ..))| {
            PatientExchangeabilityBlock::new(format!("p-{index:02}"), *block).expect("assignment")
        })
        .collect::<Vec<_>>();
    let spec = CovariatePermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 47,
        alternative: PermutationAlternative::TwoSided,
    };
    let pooled = patient_covariate_freedman_lane(&records, &spec).expect("unblocked");
    assert!((pooled.effect_group_a_minus_group_b - 10.0 / 3.0).abs() < 1e-9);
    let blocked =
        patient_blocked_covariate_freedman_lane(&records, &assignments, &spec).expect("blocked");
    assert!(blocked.effect_group_a_minus_group_b.abs() < 1e-9);
    assert!(blocked.p_value > 0.5);
    let reference = slow_reference(&records, &spec, &[(0..6).collect(), (6..12).collect()]);
    assert!((blocked.target_standard_error - reference.1).abs() < 1e-12);
    assert_eq!(blocked.p_value, reference.3);

    let by_group = records
        .iter()
        .map(|record| {
            PatientExchangeabilityBlock::new(record.patient_id.clone(), record.group.as_str())
                .expect("assignment")
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        patient_blocked_covariate_freedman_lane(&records, &by_group, &spec),
        Err(error) if error.to_string().contains("no block contains both groups")
    ));
}

#[test]
fn patient_covariate_freedman_lane_rejects_group_covariate_collinearity() {
    let records = (0..6)
        .map(|index| CovariatePatientRecord {
            patient_id: format!("p-{index}"),
            group: if index < 3 { "A" } else { "B" }.into(),
            outcome: index as f64,
            covariate: if index < 3 { 1.0 } else { 0.0 },
        })
        .collect::<Vec<_>>();
    let result = patient_covariate_freedman_lane(
        &records,
        &CovariatePermutationSpec {
            group_a: "A".into(),
            group_b: "B".into(),
            permutations: 19,
            seed: 1,
            alternative: PermutationAlternative::TwoSided,
        },
    );
    assert!(matches!(result, Err(error) if error.to_string().contains("rank deficient")));
}

fn records() -> Vec<CovariatePatientRecord> {
    let noise = [-1.0, 0.5, -1.0, 1.0, -0.5, 1.0];
    ["A", "B"]
        .into_iter()
        .flat_map(|group| {
            noise
                .into_iter()
                .enumerate()
                .map(move |(index, noise)| CovariatePatientRecord {
                    patient_id: format!("{}-{}", group.to_ascii_lowercase(), index + 1),
                    group: group.into(),
                    outcome: 2.0 * index as f64 + noise + if group == "A" { 3.0 } else { 0.0 },
                    covariate: index as f64,
                })
        })
        .collect()
}

fn slow_reference(
    records: &[CovariatePatientRecord],
    spec: &CovariatePermutationSpec,
    blocks: &[Vec<usize>],
) -> (f64, f64, f64, f64) {
    // Blocked designs absorb block means: one indicator per block after the first.
    let nuisance = records
        .iter()
        .enumerate()
        .map(|(row, record)| {
            let mut columns = vec![1.0, record.covariate];
            columns.extend(
                blocks[1..]
                    .iter()
                    .map(|block| f64::from(block.contains(&row))),
            );
            columns
        })
        .collect::<Vec<_>>();
    let y = records.iter().map(|row| row.outcome).collect::<Vec<_>>();
    let group = records
        .iter()
        .map(|row| f64::from(row.group == spec.group_a))
        .collect::<Vec<_>>();
    let fitted = qr_fitted(&nuisance, &y);
    let residuals = y
        .iter()
        .zip(&fitted)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let observed = fwl(&nuisance, &group, &y);
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
