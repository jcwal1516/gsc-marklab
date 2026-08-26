use marklab_cohort::{
    patient_level_energy_distance, EnergyDistanceSpec, EnergyMetric, Fingerprint,
};

const NAMESPACE: u64 = 0x656e_6572_6779_5f70;

#[test]
fn euclidean_energy_matches_a_slow_reference() {
    let fingerprints = [
        ("a-1", "A", [3.0, 1.0]),
        ("a-2", "A", [5.0, 2.0]),
        ("b-1", "B", [0.0, 0.0]),
        ("b-2", "B", [1.0, 1.0]),
    ]
    .into_iter()
    .map(|(patient, group, values)| Fingerprint {
        patient_id: patient.into(),
        group: group.into(),
        features: vec!["x".into(), "y".into()],
        values: values.into(),
    })
    .collect::<Vec<_>>();
    let spec = EnergyDistanceSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        metric: EnergyMetric::Euclidean,
        permutations: 199,
        seed: 53,
    };
    let result = patient_level_energy_distance(&fingerprints, &spec).expect("energy result");
    let (observed, p_value) = slow_reference(&fingerprints, &spec);
    assert!((result.energy_distance - observed).abs() < 1e-14);
    assert_eq!(result.p_value, p_value);
}

fn slow_reference(fingerprints: &[Fingerprint], spec: &EnergyDistanceSpec) -> (f64, f64) {
    let n = fingerprints.len();
    let distances = (0..n)
        .flat_map(|left| {
            (0..n).map(move |right| {
                fingerprints[left]
                    .values
                    .iter()
                    .zip(&fingerprints[right].values)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
        })
        .collect::<Vec<_>>();
    let observed_labels = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = energy(&distances, n, &observed_labels);
    let mut exceedances = 0usize;
    for replicate in 0..spec.permutations {
        let mut labels = observed_labels.clone();
        let mut state = derive_seed(spec.seed, replicate);
        for index in (1..labels.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            labels.swap(index, other);
        }
        exceedances += usize::from(energy(&distances, n, &labels) >= observed);
    }
    (
        observed,
        (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64,
    )
}

fn energy(distances: &[f64], n: usize, labels: &[bool]) -> f64 {
    let a = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| label.then_some(index))
        .collect::<Vec<_>>();
    let b = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| (!label).then_some(index))
        .collect::<Vec<_>>();
    let mean = |left: &[usize], right: &[usize]| {
        left.iter()
            .flat_map(|left| right.iter().map(move |right| distances[left * n + right]))
            .sum::<f64>()
            / (left.len() * right.len()) as f64
    };
    2.0 * mean(&a, &b) - mean(&a, &a) - mean(&b, &b)
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
