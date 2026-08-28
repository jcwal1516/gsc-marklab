use marklab_cohort::{
    patient_covariate_freedman_lane, CovariatePatientRecord, CovariatePermutationSpec,
    InferenceAlternative, InferenceNullFamily, InferencePermutationUnit, PermutationAlternative,
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
    let reference = slow_reference(&records, &spec);
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
) -> (f64, f64, f64, f64) {
    let x = records.iter().map(|row| row.covariate).collect::<Vec<_>>();
    let y = records.iter().map(|row| row.outcome).collect::<Vec<_>>();
    let group = records
        .iter()
        .map(|row| f64::from(row.group == spec.group_a))
        .collect::<Vec<_>>();
    let fitted = simple_fitted(&x, &y);
    let residuals = y
        .iter()
        .zip(&fitted)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let observed = fwl(&x, &group, &y);
    let mut lower = 0usize;
    let mut upper = 0usize;
    for replicate in 0..spec.permutations {
        let mut indices = (0..records.len()).collect::<Vec<_>>();
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
        let statistic = fwl(&x, &group, &permuted).2;
        lower += usize::from(statistic <= observed.2);
        upper += usize::from(statistic >= observed.2);
    }
    let p = (2.0 * ((lower.min(upper) as f64 + 1.0) / (spec.permutations + 1) as f64)).min(1.0);
    (observed.0, observed.1, observed.2, p)
}

fn simple_fitted(x: &[f64], y: &[f64]) -> Vec<f64> {
    let mean_x = x.iter().sum::<f64>() / x.len() as f64;
    let mean_y = y.iter().sum::<f64>() / y.len() as f64;
    let slope = x
        .iter()
        .zip(y)
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>()
        / x.iter().map(|x| (x - mean_x).powi(2)).sum::<f64>();
    x.iter().map(|x| mean_y + slope * (x - mean_x)).collect()
}

fn fwl(x: &[f64], group: &[f64], y: &[f64]) -> (f64, f64, f64) {
    let group_fit = simple_fitted(x, group);
    let y_fit = simple_fitted(x, y);
    let group_residual = group
        .iter()
        .zip(group_fit)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let y_residual = y
        .iter()
        .zip(y_fit)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let denominator = group_residual
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    let coefficient = group_residual
        .iter()
        .zip(&y_residual)
        .map(|(group, outcome)| group * outcome)
        .sum::<f64>()
        / denominator;
    let rss = y_residual
        .iter()
        .zip(&group_residual)
        .map(|(outcome, group)| (outcome - coefficient * group).powi(2))
        .sum::<f64>();
    let standard_error = (rss / (y.len() - 3) as f64 / denominator).sqrt();
    (coefficient, standard_error, coefficient / standard_error)
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
