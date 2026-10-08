use marklab_cohort::{
    paired_patient_permutation_test, InferenceNullFamily, InferencePermutationUnit,
    PairedPatientEndpoint, PairedPatientPermutationSpec, PermutationAlternative,
};

const NAMESPACE: u64 = 0x7061_6972_5f73_6967;

#[test]
fn paired_sign_flip_matches_a_slow_reference() {
    let differences = [2.0, 3.0, 4.0, 5.0];
    let records = differences
        .iter()
        .enumerate()
        .flat_map(|(index, difference)| {
            let patient_id = format!("p-{}", index + 1);
            [
                PairedPatientEndpoint {
                    patient_id: patient_id.clone(),
                    condition: "A".into(),
                    endpoint: index as f64 + 1.0,
                },
                PairedPatientEndpoint {
                    patient_id,
                    condition: "B".into(),
                    endpoint: index as f64 + 1.0 + difference,
                },
            ]
        })
        .collect::<Vec<_>>();
    let spec = PairedPatientPermutationSpec {
        condition_a: "A".into(),
        condition_b: "B".into(),
        permutations: 199,
        seed: 23,
        alternative: PermutationAlternative::Greater,
    };

    let result = paired_patient_permutation_test(&records, &spec).expect("paired result");
    assert_eq!(result.p_value, slow_reference_p_value(&differences, &spec));
    assert_eq!(
        result.inference_design.null_family(),
        InferenceNullFamily::PairedSignFlip
    );
    assert_eq!(
        result.inference_design.permutation_unit(),
        InferencePermutationUnit::CompletePatientPairDifference
    );
    assert_eq!(result.inference_design.alternative(), spec.alternative);
}

fn slow_reference_p_value(differences: &[f64], spec: &PairedPatientPermutationSpec) -> f64 {
    let observed = studentized(differences);
    let mut lower = 0usize;
    let mut upper = 0usize;
    for replicate in 0..spec.permutations {
        let mut state = derive_seed(spec.seed, replicate);
        let signed = differences
            .iter()
            .enumerate()
            .map(|(index, difference)| {
                state = splitmix64(state ^ index as u64);
                if state & 1 == 0 {
                    *difference
                } else {
                    -*difference
                }
            })
            .collect::<Vec<_>>();
        let statistic = studentized(&signed);
        lower += usize::from(statistic <= observed);
        upper += usize::from(statistic >= observed);
    }
    let denominator = (spec.permutations + 1) as f64;
    match spec.alternative {
        PermutationAlternative::Less => (lower as f64 + 1.0) / denominator,
        PermutationAlternative::Greater => (upper as f64 + 1.0) / denominator,
        PermutationAlternative::TwoSided => {
            (2.0 * ((lower.min(upper) as f64 + 1.0) / denominator)).min(1.0)
        }
    }
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
