use std::collections::BTreeMap;

use marklab_cohort::{
    patient_level_permutation_test, CohortInferenceError, PatientEndpoint, PatientPermutationSpec,
    PermutationAlternative,
};

const NAMESPACE: u64 = 0x7061_7469_656e_745f;

#[test]
fn blocked_and_unblocked_results_match_a_slow_reference() {
    let base = vec![
        endpoint("a-1", "A", 8.0, None),
        endpoint("a-2", "A", 9.0, None),
        endpoint("a-3", "A", 12.0, None),
        endpoint("a-4", "A", 13.0, None),
        endpoint("b-1", "B", 1.0, None),
        endpoint("b-2", "B", 2.0, None),
        endpoint("b-3", "B", 4.0, None),
        endpoint("b-4", "B", 5.0, None),
    ];
    let spec = PatientPermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 91,
        alternative: PermutationAlternative::TwoSided,
    };

    let unblocked = patient_level_permutation_test(&base, &spec).expect("unblocked result");
    assert_eq!(unblocked.p_value, slow_reference_p_value(&base, &spec));

    let mut blocked = base;
    for (index, row) in blocked.iter_mut().enumerate() {
        row.block = Some(
            if index.is_multiple_of(2) {
                "north"
            } else {
                "south"
            }
            .into(),
        );
    }
    let blocked_result = patient_level_permutation_test(&blocked, &spec).expect("blocked result");
    assert!(blocked_result.blocked);
    assert_eq!(blocked_result.block_count, 2);
    assert_eq!(
        blocked_result.p_value,
        slow_reference_p_value(&blocked, &spec)
    );
}

#[test]
fn partially_declared_hierarchy_blocks_are_rejected() {
    let mut records = vec![
        endpoint("a-1", "A", 8.0, None),
        endpoint("a-2", "A", 9.0, None),
        endpoint("a-3", "A", 12.0, None),
        endpoint("a-4", "A", 13.0, None),
        endpoint("b-1", "B", 1.0, None),
        endpoint("b-2", "B", 2.0, None),
        endpoint("b-3", "B", 4.0, None),
        endpoint("b-4", "B", 5.0, None),
    ];
    records[0].block = Some("north".into());
    records[4].block = Some("north".into());
    let error = patient_level_permutation_test(
        &records,
        &PatientPermutationSpec {
            group_a: "A".into(),
            group_b: "B".into(),
            permutations: 19,
            seed: 7,
            alternative: PermutationAlternative::TwoSided,
        },
    )
    .expect_err("partial hierarchy blocks must fail");
    assert_eq!(
        error,
        CohortInferenceError::InvalidInput(
            "exchangeability blocks must be declared for every patient or no patients".into()
        )
    );
}

fn endpoint(patient_id: &str, group: &str, value: f64, block: Option<&str>) -> PatientEndpoint {
    PatientEndpoint {
        patient_id: patient_id.into(),
        group: group.into(),
        endpoint: value,
        block: block.map(str::to_owned),
    }
}

fn slow_reference_p_value(records: &[PatientEndpoint], spec: &PatientPermutationSpec) -> f64 {
    let observed_labels = records
        .iter()
        .map(|row| row.group == spec.group_a)
        .collect::<Vec<_>>();
    let observed = slow_studentized(records, &observed_labels);
    let mut lower = 0usize;
    let mut upper = 0usize;
    for replicate in 0..spec.permutations {
        let mut labels = observed_labels.clone();
        let mut blocks = BTreeMap::<&str, Vec<usize>>::new();
        for (index, row) in records.iter().enumerate() {
            blocks
                .entry(row.block.as_deref().unwrap_or_default())
                .or_default()
                .push(index);
        }
        let mut state = derive_seed(spec.seed, replicate);
        for indices in blocks.values() {
            let mut values = indices
                .iter()
                .map(|index| labels[*index])
                .collect::<Vec<_>>();
            for index in (1..values.len()).rev() {
                state = splitmix64(state ^ index as u64);
                let other = (state % (index as u64 + 1)) as usize;
                values.swap(index, other);
            }
            for (index, value) in indices.iter().zip(values) {
                labels[*index] = value;
            }
        }
        let statistic = slow_studentized(records, &labels);
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

fn slow_studentized(records: &[PatientEndpoint], labels: &[bool]) -> f64 {
    let a = records
        .iter()
        .zip(labels)
        .filter_map(|(row, label)| label.then_some(row.endpoint))
        .collect::<Vec<_>>();
    let b = records
        .iter()
        .zip(labels)
        .filter_map(|(row, label)| (!label).then_some(row.endpoint))
        .collect::<Vec<_>>();
    let mean_a = a.iter().sum::<f64>() / a.len() as f64;
    let mean_b = b.iter().sum::<f64>() / b.len() as f64;
    let variance_a =
        a.iter().map(|value| (value - mean_a).powi(2)).sum::<f64>() / (a.len() - 1) as f64;
    let variance_b =
        b.iter().map(|value| (value - mean_b).powi(2)).sum::<f64>() / (b.len() - 1) as f64;
    (mean_a - mean_b) / (variance_a / a.len() as f64 + variance_b / b.len() as f64).sqrt()
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
