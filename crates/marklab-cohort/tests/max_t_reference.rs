use marklab_cohort::{
    max_t_multiple_endpoint_permutation, MaxTPermutationSpec, PatientEndpointVector,
};

const NAMESPACE: u64 = 0x6d61_785f_745f_7065;

#[test]
fn max_t_adjustment_and_critical_value_match_a_slow_reference() {
    let patients = [
        ("a-1", "A", [8.0, 4.0, 2.0]),
        ("a-2", "A", [9.0, 7.0, 5.0]),
        ("b-1", "B", [1.0, 2.0, 3.0]),
        ("b-2", "B", [2.0, 3.0, 4.0]),
    ]
    .into_iter()
    .map(|(patient, group, values)| PatientEndpointVector {
        patient_id: patient.into(),
        group: group.into(),
        endpoints: vec!["e1".into(), "e2".into(), "e3".into()],
        values: values.into(),
    })
    .collect::<Vec<_>>();
    let spec = MaxTPermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 41,
        alpha: 0.05,
    };
    let result = max_t_multiple_endpoint_permutation(&patients, &spec).expect("Max-T result");
    let (adjusted, critical) = slow_reference(&patients, &spec);
    assert_eq!(
        result
            .endpoints
            .iter()
            .map(|endpoint| endpoint.adjusted_p_value)
            .collect::<Vec<_>>(),
        adjusted
    );
    assert_eq!(result.critical_value, critical);
}

fn slow_reference(
    patients: &[PatientEndpointVector],
    spec: &MaxTPermutationSpec,
) -> (Vec<f64>, f64) {
    let observed_labels = patients
        .iter()
        .map(|patient| patient.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = statistics(patients, &observed_labels);
    let mut maxima = Vec::with_capacity(spec.permutations);
    for replicate in 0..spec.permutations {
        let mut labels = observed_labels.clone();
        let mut state = derive_seed(spec.seed, replicate);
        for index in (1..labels.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            labels.swap(index, other);
        }
        maxima.push(
            statistics(patients, &labels)
                .into_iter()
                .map(f64::abs)
                .fold(0.0_f64, f64::max),
        );
    }
    let adjusted = observed
        .iter()
        .map(|statistic| {
            (maxima
                .iter()
                .filter(|maximum| **maximum >= statistic.abs())
                .count() as f64
                + 1.0)
                / (spec.permutations + 1) as f64
        })
        .collect();
    maxima.sort_by(f64::total_cmp);
    let rank = ((1.0 - spec.alpha) * (spec.permutations + 1) as f64).ceil() as usize;
    let critical = maxima[rank.saturating_sub(1).min(maxima.len() - 1)];
    (adjusted, critical)
}

fn statistics(patients: &[PatientEndpointVector], labels: &[bool]) -> Vec<f64> {
    (0..patients[0].values.len())
        .map(|endpoint| {
            let a = patients
                .iter()
                .zip(labels)
                .filter_map(|(patient, label)| label.then_some(patient.values[endpoint]))
                .collect::<Vec<_>>();
            let b = patients
                .iter()
                .zip(labels)
                .filter_map(|(patient, label)| (!label).then_some(patient.values[endpoint]))
                .collect::<Vec<_>>();
            let mean_a = a.iter().sum::<f64>() / a.len() as f64;
            let mean_b = b.iter().sum::<f64>() / b.len() as f64;
            let variance_a =
                a.iter().map(|value| (value - mean_a).powi(2)).sum::<f64>() / (a.len() - 1) as f64;
            let variance_b =
                b.iter().map(|value| (value - mean_b).powi(2)).sum::<f64>() / (b.len() - 1) as f64;
            (mean_a - mean_b) / (variance_a / a.len() as f64 + variance_b / b.len() as f64).sqrt()
        })
        .collect()
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
