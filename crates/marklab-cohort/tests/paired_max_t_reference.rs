use marklab_cohort::{
    paired_max_t_permutation, CohortInferenceError, InferenceAlternative, InferenceMultiplicity,
    InferenceNullFamily, InferencePermutationUnit, MaxTCorrection, PairedMaxTPermutationSpec,
    PairedPatientEndpointVector,
};

const NAMESPACE: u64 = 0x7061_6972_6d61_7874;

#[test]
fn paired_single_step_and_step_down_max_t_match_a_slow_vector_sign_oracle() {
    let records = records();
    for correction in [MaxTCorrection::SingleStep, MaxTCorrection::StepDown] {
        let spec = spec(correction);
        let result = paired_max_t_permutation(&records, &spec).expect("paired Max-T");
        let (adjusted, critical) = slow_reference(&records, &spec);
        assert_eq!(
            result
                .endpoints
                .iter()
                .map(|endpoint| endpoint.adjusted_p_value)
                .collect::<Vec<_>>(),
            adjusted
        );
        assert_eq!(result.critical_value, critical);
        assert_eq!(
            result.inference_design.alternative(),
            InferenceAlternative::TwoSided
        );
        assert_eq!(
            result.inference_design.null_family(),
            InferenceNullFamily::PairedSignFlip
        );
        assert_eq!(
            result.inference_design.multiplicity(),
            InferenceMultiplicity::CompleteEndpointFamilyMaxT
        );
        assert_eq!(
            result.inference_design.permutation_unit(),
            InferencePermutationUnit::CompletePatientPairDifferenceVector
        );
    }
}

#[test]
fn paired_max_t_rejects_an_incomplete_endpoint_family() {
    let mut incomplete = records();
    incomplete[1].endpoints.pop();
    incomplete[1].values.pop();
    assert!(matches!(
        paired_max_t_permutation(&incomplete, &spec(MaxTCorrection::SingleStep)),
        Err(CohortInferenceError::InvalidInput(message))
            if message.contains("exact complete paired endpoint family")
    ));
}

fn records() -> Vec<PairedPatientEndpointVector> {
    [
        ("p-1", [1.0, 4.0, 2.0], [3.0, 5.0, 2.5]),
        ("p-2", [2.0, 3.0, 5.0], [5.0, 5.0, 5.5]),
        ("p-3", [3.0, 2.0, 3.0], [7.0, 5.0, 4.0]),
        ("p-4", [4.0, 1.0, 4.0], [9.0, 5.0, 4.5]),
    ]
    .into_iter()
    .flat_map(|(patient, a, b)| {
        [
            PairedPatientEndpointVector {
                patient_id: patient.into(),
                condition: "A".into(),
                endpoints: vec!["middle".into(), "strong".into(), "weak".into()],
                values: vec![a[1], a[0], a[2]],
            },
            PairedPatientEndpointVector {
                patient_id: patient.into(),
                condition: "B".into(),
                endpoints: vec!["middle".into(), "strong".into(), "weak".into()],
                values: vec![b[1], b[0], b[2]],
            },
        ]
    })
    .collect()
}

fn spec(correction: MaxTCorrection) -> PairedMaxTPermutationSpec {
    PairedMaxTPermutationSpec {
        condition_a: "A".into(),
        condition_b: "B".into(),
        permutations: 199,
        seed: 23,
        alpha: 0.05,
        correction,
    }
}

fn slow_reference(
    records: &[PairedPatientEndpointVector],
    spec: &PairedMaxTPermutationSpec,
) -> (Vec<f64>, f64) {
    let pair_count = records.len() / 2;
    let endpoint_count = records[0].values.len();
    let differences = (0..endpoint_count)
        .map(|endpoint| {
            (0..pair_count)
                .map(|pair| {
                    records[2 * pair + 1].values[endpoint] - records[2 * pair].values[endpoint]
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let observed = differences
        .iter()
        .map(|values| studentized(values))
        .collect::<Vec<_>>();
    let mut order = (0..endpoint_count).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        observed[*right]
            .abs()
            .total_cmp(&observed[*left].abs())
            .then_with(|| left.cmp(right))
    });
    let mut null_rows = Vec::with_capacity(spec.permutations);
    for replicate in 0..spec.permutations {
        let mut state = derive_seed(spec.seed, replicate);
        let signs = (0..pair_count)
            .map(|pair| {
                state = splitmix64(state ^ pair as u64);
                if state & 1 == 0 {
                    1.0
                } else {
                    -1.0
                }
            })
            .collect::<Vec<_>>();
        null_rows.push(
            differences
                .iter()
                .map(|values| {
                    studentized(
                        &values
                            .iter()
                            .zip(&signs)
                            .map(|(value, sign)| value * sign)
                            .collect::<Vec<_>>(),
                    )
                    .abs()
                })
                .collect::<Vec<_>>(),
        );
    }
    let mut adjusted = vec![0.0; endpoint_count];
    for (rank, endpoint) in order.iter().enumerate() {
        let exceedances = null_rows
            .iter()
            .filter(|row| {
                let maximum = match spec.correction {
                    MaxTCorrection::SingleStep => row.iter().copied().fold(0.0_f64, f64::max),
                    MaxTCorrection::StepDown => order[rank..]
                        .iter()
                        .map(|remaining| row[*remaining])
                        .fold(0.0_f64, f64::max),
                };
                maximum >= observed[*endpoint].abs()
            })
            .count();
        adjusted[*endpoint] = (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64;
    }
    if spec.correction == MaxTCorrection::StepDown {
        let mut previous = 0.0_f64;
        for endpoint in order {
            previous = previous.max(adjusted[endpoint]);
            adjusted[endpoint] = previous;
        }
    }
    let mut maxima = null_rows
        .iter()
        .map(|row| row.iter().copied().fold(0.0_f64, f64::max))
        .collect::<Vec<_>>();
    maxima.sort_by(f64::total_cmp);
    let rank = ((1.0 - spec.alpha) * (spec.permutations + 1) as f64).ceil() as usize;
    let critical = maxima[rank.saturating_sub(1).min(maxima.len() - 1)];
    (adjusted, critical)
}

fn studentized(values: &[f64]) -> f64 {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    mean / (variance / values.len() as f64).sqrt()
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
