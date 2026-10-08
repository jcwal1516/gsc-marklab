use marklab_cohort::{
    patient_level_blocked_mmd, patient_level_mmd, Fingerprint, InferenceAnalysisLevel,
    InferenceNullFamily, InferencePermutationUnit, MmdEstimator, MmdKernel, MmdPermutationSpec,
    PatientExchangeabilityBlock,
};

const NAMESPACE: u64 = 0x6d6d_645f_7065_726d;

#[test]
fn linear_and_rbf_mmd_match_a_slow_reference() {
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
    for kernel in [MmdKernel::Linear, MmdKernel::Rbf { bandwidth: 2.0 }] {
        let spec = MmdPermutationSpec {
            group_a: "A".into(),
            group_b: "B".into(),
            kernel,
            estimator: MmdEstimator::Unbiased,
            permutations: 199,
            seed: 47,
        };
        let result = patient_level_mmd(&fingerprints, &spec).expect("MMD result");
        let (observed, p_value) = slow_reference(&fingerprints, &spec);
        assert!((result.mmd_squared - observed).abs() < 1e-14);
        assert_eq!(result.p_value, p_value);
    }
}

#[test]
fn blocked_mmd_matches_a_patient_id_keyed_slow_reference() {
    let fingerprints = blocked_fingerprints();
    let assignments = blocked_assignments();
    let spec = MmdPermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        kernel: MmdKernel::Linear,
        estimator: MmdEstimator::Unbiased,
        permutations: 199,
        seed: 47,
    };
    let blocked =
        patient_level_blocked_mmd(&fingerprints, &assignments, &spec).expect("blocked MMD result");
    let (observed, p_value) = slow_blocked_reference(&fingerprints, &spec);

    assert!((blocked.result().mmd_squared - observed).abs() < 1e-14);
    assert_eq!(blocked.result().p_value, p_value);
    assert_eq!(
        blocked.design().analysis_level(),
        InferenceAnalysisLevel::Patient
    );
    assert_eq!(
        blocked.design().null_family(),
        InferenceNullFamily::PopulationIndependence
    );
    assert_eq!(
        blocked.design().permutation_unit(),
        InferencePermutationUnit::PatientLabel
    );
    assert_eq!(blocked.design().block_count(), 2);
    assert_eq!(blocked.design().unit_count(), 8);
}

#[test]
fn blocked_mmd_rejects_incomplete_duplicate_foreign_and_confounded_assignments() {
    let fingerprints = blocked_fingerprints();
    let spec = MmdPermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        kernel: MmdKernel::Linear,
        estimator: MmdEstimator::Unbiased,
        permutations: 19,
        seed: 47,
    };
    let incomplete = &blocked_assignments()[1..];
    assert!(patient_level_blocked_mmd(&fingerprints, incomplete, &spec).is_err());

    let mut duplicate = blocked_assignments();
    duplicate[0] = duplicate[1].clone();
    assert!(patient_level_blocked_mmd(&fingerprints, &duplicate, &spec).is_err());

    let mut foreign = blocked_assignments();
    foreign[0] = PatientExchangeabilityBlock::new("foreign", "north").expect("assignment");
    assert!(patient_level_blocked_mmd(&fingerprints, &foreign, &spec).is_err());

    let confounded = fingerprints
        .iter()
        .map(|fingerprint| {
            PatientExchangeabilityBlock::new(&fingerprint.patient_id, &fingerprint.group)
                .expect("assignment")
        })
        .collect::<Vec<_>>();
    assert!(patient_level_blocked_mmd(&fingerprints, &confounded, &spec).is_err());
}

fn blocked_fingerprints() -> Vec<Fingerprint> {
    [
        ("a-1", "A", 3.0),
        ("a-2", "A", 5.0),
        ("b-1", "B", 0.0),
        ("b-2", "B", 1.0),
        ("a-3", "A", 4.0),
        ("a-4", "A", 6.0),
        ("b-3", "B", 1.0),
        ("b-4", "B", 2.0),
    ]
    .into_iter()
    .map(|(patient_id, group, value)| Fingerprint {
        patient_id: patient_id.into(),
        group: group.into(),
        features: vec!["x".into()],
        values: vec![value],
    })
    .collect()
}

fn blocked_assignments() -> Vec<PatientExchangeabilityBlock> {
    [
        ("b-4", "south"),
        ("b-3", "south"),
        ("a-4", "south"),
        ("a-3", "south"),
        ("b-2", "north"),
        ("b-1", "north"),
        ("a-2", "north"),
        ("a-1", "north"),
    ]
    .into_iter()
    .map(|(patient_id, block)| {
        PatientExchangeabilityBlock::new(patient_id, block).expect("assignment")
    })
    .collect()
}

fn slow_blocked_reference(fingerprints: &[Fingerprint], spec: &MmdPermutationSpec) -> (f64, f64) {
    let n = fingerprints.len();
    let kernel = (0..n)
        .flat_map(|left| {
            (0..n).map(move |right| fingerprints[left].values[0] * fingerprints[right].values[0])
        })
        .collect::<Vec<_>>();
    let observed_labels = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = mmd(&kernel, n, &observed_labels);
    let mut exceedances = 0usize;
    for replicate in 0..spec.permutations {
        let labels = blocked_labels(&observed_labels, spec.seed, replicate);
        exceedances += usize::from(mmd(&kernel, n, &labels) >= observed);
    }
    (
        observed,
        (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64,
    )
}

fn blocked_labels(observed: &[bool], seed: u64, replicate: usize) -> Vec<bool> {
    let mut labels = observed.to_vec();
    let mut state = derive_seed(seed, replicate);
    for block in [[0, 1, 2, 3], [4, 5, 6, 7]] {
        let mut sources = block;
        for index in (1..sources.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            sources.swap(index, other);
        }
        for (position, source) in block.into_iter().zip(sources) {
            labels[position] = observed[source];
        }
    }
    labels
}

fn slow_reference(fingerprints: &[Fingerprint], spec: &MmdPermutationSpec) -> (f64, f64) {
    let n = fingerprints.len();
    let kernel = (0..n)
        .flat_map(|left| {
            (0..n).map(move |right| match spec.kernel {
                MmdKernel::Linear => fingerprints[left]
                    .values
                    .iter()
                    .zip(&fingerprints[right].values)
                    .map(|(a, b)| a * b)
                    .sum(),
                MmdKernel::Rbf { bandwidth } => {
                    let squared_distance = fingerprints[left]
                        .values
                        .iter()
                        .zip(&fingerprints[right].values)
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f64>();
                    (-squared_distance / (2.0 * bandwidth.powi(2))).exp()
                }
            })
        })
        .collect::<Vec<_>>();
    let observed_labels = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = mmd(&kernel, n, &observed_labels);
    let mut exceedances = 0usize;
    for replicate in 0..spec.permutations {
        let mut labels = observed_labels.clone();
        let mut state = derive_seed(spec.seed, replicate);
        for index in (1..labels.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            labels.swap(index, other);
        }
        exceedances += usize::from(mmd(&kernel, n, &labels) >= observed);
    }
    (
        observed,
        (exceedances as f64 + 1.0) / (spec.permutations + 1) as f64,
    )
}

fn mmd(kernel: &[f64], n: usize, labels: &[bool]) -> f64 {
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
    let within = |indices: &[usize]| {
        indices
            .iter()
            .flat_map(|left| {
                indices
                    .iter()
                    .filter_map(move |right| (left != right).then_some(kernel[left * n + right]))
            })
            .sum::<f64>()
            / (indices.len() * (indices.len() - 1)) as f64
    };
    let cross = a
        .iter()
        .flat_map(|left| b.iter().map(move |right| kernel[left * n + right]))
        .sum::<f64>()
        / (a.len() * b.len()) as f64;
    within(&a) + within(&b) - 2.0 * cross
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
