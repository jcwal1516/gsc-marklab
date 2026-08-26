use marklab_cohort::{
    functional_two_sample_permutation, FunctionalCurve, FunctionalPermutationSpec,
    FunctionalTestStatistic,
};

const NAMESPACE: u64 = 0x6675_6e63_5f70_6572;

#[test]
fn functional_l2_matches_a_slow_reference() {
    let curves = [
        ("a-1", "A", [2.0, 3.0, 5.0]),
        ("a-2", "A", [4.0, 5.0, 7.0]),
        ("b-1", "B", [0.0, 1.0, 1.0]),
        ("b-2", "B", [1.0, 2.0, 2.0]),
    ]
    .into_iter()
    .map(|(patient, group, values)| FunctionalCurve {
        patient_id: patient.into(),
        group: group.into(),
        axis: vec![0.0, 1.0, 3.0],
        values: values.into(),
    })
    .collect::<Vec<_>>();
    let spec = FunctionalPermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        statistic: FunctionalTestStatistic::L2,
        permutations: 199,
        seed: 31,
    };
    let result = functional_two_sample_permutation(&curves, &spec).expect("functional result");
    assert_eq!(result.p_value, slow_reference_p_value(&curves, &spec));
}

fn slow_reference_p_value(curves: &[FunctionalCurve], spec: &FunctionalPermutationSpec) -> f64 {
    let observed_labels = curves
        .iter()
        .map(|curve| curve.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = slow_l2(curves, &observed_labels);
    let mut exceedances = 0usize;
    for replicate in 0..spec.permutations {
        let mut labels = observed_labels.clone();
        let mut state = derive_seed(spec.seed, replicate);
        for index in (1..labels.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            labels.swap(index, other);
        }
        exceedances += usize::from(slow_l2(curves, &labels) >= observed);
    }
    (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64
}

fn slow_l2(curves: &[FunctionalCurve], labels: &[bool]) -> f64 {
    let count_a = labels.iter().filter(|label| **label).count();
    let count_b = labels.len() - count_a;
    let difference = (0..curves[0].axis.len())
        .map(|axis_index| {
            let sum_a = curves
                .iter()
                .zip(labels)
                .filter_map(|(curve, label)| label.then_some(curve.values[axis_index]))
                .sum::<f64>();
            let sum_b = curves
                .iter()
                .zip(labels)
                .filter_map(|(curve, label)| (!label).then_some(curve.values[axis_index]))
                .sum::<f64>();
            sum_a / count_a as f64 - sum_b / count_b as f64
        })
        .collect::<Vec<_>>();
    curves[0]
        .axis
        .windows(2)
        .zip(difference.windows(2))
        .map(|(axis, values)| (axis[1] - axis[0]) * (values[0].powi(2) + values[1].powi(2)) / 2.0)
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
