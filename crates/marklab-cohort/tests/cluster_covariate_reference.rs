use marklab_cohort::{
    cluster_covariate_matrix_freedman_lane, ClusterCovariatePatientRecord,
    CovariatePermutationSpec, InferenceAnalysisLevel, InferenceNullFamily,
    InferencePermutationUnit, PermutationAlternative,
};

const NAMESPACE: u64 = 0x636c_7573_636f_765f;

#[test]
fn cluster_covariate_result_matches_cluster_qr_fwl_oracle() {
    let records = records();
    let spec = spec();
    let result = cluster_covariate_matrix_freedman_lane(&records, &spec).expect("cluster result");
    let reference = slow_reference(&spec);
    assert!((result.effect_group_a_minus_group_b - reference.0).abs() < 1e-12);
    assert!((result.target_standard_error - reference.1).abs() < 1e-12);
    assert!((result.studentized_statistic - reference.2).abs() < 1e-12);
    assert_eq!(result.p_value, reference.3);
    assert_eq!(result.patient_count, 10);
    assert_eq!(result.cluster_count, 8);
    assert_eq!(result.group_a_cluster_count, 4);
    assert_eq!(result.group_b_cluster_count, 4);
    assert_eq!(
        result.inference_design.analysis_level(),
        InferenceAnalysisLevel::Cluster
    );
    assert_eq!(
        result.inference_design.null_family(),
        InferenceNullFamily::ClusterCovariateResidualPermutation
    );
    assert_eq!(
        result.inference_design.permutation_unit(),
        InferencePermutationUnit::CompleteClusterResidual
    );

    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(
        result,
        cluster_covariate_matrix_freedman_lane(&reversed, &spec).expect("reordered result")
    );
    let mut rescaled = records.clone();
    for record in &mut rescaled {
        record.covariates[0] *= 1e100;
        record.covariates[1] *= 1e-100;
    }
    let scaled = cluster_covariate_matrix_freedman_lane(&rescaled, &spec).expect("scaled result");
    assert!(
        (result.effect_group_a_minus_group_b - scaled.effect_group_a_minus_group_b).abs() < 1e-12
    );
    assert_eq!(result.p_value, scaled.p_value);
}

#[test]
fn cluster_covariate_rejects_broken_hierarchy_matrix_and_rank() {
    let mut duplicate = records();
    duplicate[9].patient_id = duplicate[0].patient_id.clone();
    assert!(cluster_covariate_matrix_freedman_lane(&duplicate, &spec()).is_err());

    let mut conflicting = records();
    conflicting[5].cluster_id = "a1".into();
    assert!(matches!(
        cluster_covariate_matrix_freedman_lane(&conflicting, &spec()),
        Err(error) if error.to_string().contains("conflicting group")
    ));

    let mut missing = records();
    missing[0].covariate_names.pop();
    missing[0].covariates.pop();
    assert!(cluster_covariate_matrix_freedman_lane(&missing, &spec()).is_err());

    let mut collinear = records();
    for record in &mut collinear {
        record.covariates[1] = 2.0 * record.covariates[0];
    }
    assert!(matches!(
        cluster_covariate_matrix_freedman_lane(&collinear, &spec()),
        Err(error) if error.to_string().contains("rank deficient")
    ));

    let too_few_a = records()
        .into_iter()
        .filter(|record| record.group == "B" || record.cluster_id == "a1")
        .collect::<Vec<_>>();
    assert!(matches!(
        cluster_covariate_matrix_freedman_lane(&too_few_a, &spec()),
        Err(error) if error.to_string().contains("two independent clusters")
    ));
}

fn records() -> Vec<ClusterCovariatePatientRecord> {
    [
        ("a1-1", "a1", "A", 4.0, -3.0, -1.0),
        ("a1-2", "a1", "A", 6.0, -1.0, 1.0),
        ("a2", "a2", "A", 5.0, -1.0, -1.0),
        ("a3", "a3", "A", 7.0, 1.0, 1.0),
        ("a4", "a4", "A", 7.0, 3.0, -1.0),
        ("b1", "b1", "B", 0.0, -3.0, 1.0),
        ("b2", "b2", "B", 2.0, -1.0, -1.0),
        ("b3", "b3", "B", 2.0, 1.0, 1.0),
        ("b4-1", "b4", "B", 3.0, 2.0, -1.0),
        ("b4-2", "b4", "B", 5.0, 4.0, 1.0),
    ]
    .into_iter()
    .map(
        |(patient_id, cluster_id, group, outcome, age, batch)| ClusterCovariatePatientRecord {
            patient_id: patient_id.into(),
            cluster_id: cluster_id.into(),
            group: group.into(),
            outcome,
            covariate_names: vec!["age".into(), "batch".into()],
            covariates: vec![age, batch],
        },
    )
    .collect()
}

fn spec() -> CovariatePermutationSpec {
    CovariatePermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 71,
        alternative: PermutationAlternative::TwoSided,
    }
}

fn slow_reference(spec: &CovariatePermutationSpec) -> (f64, f64, f64, f64) {
    let summaries = [
        (5.0, -2.0, 0.0, 1.0),
        (5.0, -1.0, -1.0, 1.0),
        (7.0, 1.0, 1.0, 1.0),
        (7.0, 3.0, -1.0, 1.0),
        (0.0, -3.0, 1.0, 0.0),
        (2.0, -1.0, -1.0, 0.0),
        (2.0, 1.0, 1.0, 0.0),
        (4.0, 3.0, 0.0, 0.0),
    ];
    let nuisance = summaries
        .iter()
        .map(|(_, age, batch, _)| vec![1.0, *age, *batch])
        .collect::<Vec<_>>();
    let outcome = summaries
        .iter()
        .map(|(outcome, _, _, _)| *outcome)
        .collect::<Vec<_>>();
    let group = summaries
        .iter()
        .map(|(_, _, _, group)| *group)
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
        let mut indices = (0..summaries.len()).collect::<Vec<_>>();
        let mut state = derive_seed(spec.seed, replicate);
        for index in (1..indices.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            indices.swap(index, other);
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
