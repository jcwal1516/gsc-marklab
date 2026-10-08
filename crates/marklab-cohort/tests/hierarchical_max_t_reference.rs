use marklab_cohort::{
    hierarchical_gatekeeping_max_t, InferenceMultiplicity, MaxTCorrection, MaxTPermutationSpec,
    OrderedEndpointFamily, PatientEndpointVector,
};

const NAMESPACE: u64 = 0x6869_6572_6d61_7874;

#[test]
fn hierarchical_gatekeeping_matches_independent_local_max_t_oracles() {
    let patients = patients();
    let families = families();
    let spec = spec();
    for correction in [MaxTCorrection::SingleStep, MaxTCorrection::StepDown] {
        let result = hierarchical_gatekeeping_max_t(&patients, &families, &spec, correction)
            .expect("hierarchical result");
        let reference = slow_reference(&patients, &families, &spec, correction);
        assert_eq!(
            result.inference_design.multiplicity(),
            InferenceMultiplicity::OrderedFamilyGatekeepingMaxT
        );
        assert_eq!(result.opened_family_count, reference.opened_family_count);
        for (actual, expected) in result.families.iter().zip(reference.families) {
            assert_eq!(actual.opened, expected.opened);
            assert_eq!(actual.all_endpoints_rejected, expected.all_rejected);
            assert_eq!(actual.critical_value, expected.critical_value);
            assert_eq!(
                actual
                    .endpoints
                    .iter()
                    .map(|endpoint| endpoint.local_adjusted_p_value)
                    .collect::<Vec<_>>(),
                expected.adjusted_p_values
            );
            assert_eq!(
                actual
                    .endpoints
                    .iter()
                    .map(|endpoint| endpoint.rejected)
                    .collect::<Vec<_>>(),
                expected.rejected
            );
        }
    }
}

#[test]
fn primary_failure_closes_descendants_and_invalid_partitions_fail() {
    let mut closed_patients = patients();
    for (index, patient) in closed_patients.iter_mut().enumerate() {
        patient.values[1] = (index % 4) as f64;
    }
    let result = hierarchical_gatekeeping_max_t(
        &closed_patients,
        &families(),
        &spec(),
        MaxTCorrection::StepDown,
    )
    .expect("closed hierarchy");
    assert!(result.families[0].opened);
    assert!(!result.families[0].all_endpoints_rejected);
    assert!(!result.families[1].opened);
    assert!(result.families[1]
        .endpoints
        .iter()
        .all(|endpoint| !endpoint.rejected));

    let mut overlap = families();
    overlap[1].endpoints[0] = "response".into();
    assert!(hierarchical_gatekeeping_max_t(
        &patients(),
        &overlap,
        &spec(),
        MaxTCorrection::SingleStep
    )
    .is_err());
    assert!(hierarchical_gatekeeping_max_t(
        &patients(),
        &families()[..1],
        &spec(),
        MaxTCorrection::SingleStep
    )
    .is_err());
}

fn patients() -> Vec<PatientEndpointVector> {
    [
        ("a1", "A", [8.0, 7.0, 5.0]),
        ("a2", "A", [9.0, 8.0, 6.0]),
        ("a3", "A", [10.0, 9.0, 7.0]),
        ("a4", "A", [11.0, 10.0, 8.0]),
        ("b1", "B", [1.0, 1.5, 4.0]),
        ("b2", "B", [2.0, 2.5, 4.5]),
        ("b3", "B", [3.0, 3.5, 5.0]),
        ("b4", "B", [4.0, 4.5, 5.5]),
    ]
    .into_iter()
    .map(|(patient_id, group, values)| PatientEndpointVector {
        patient_id: patient_id.into(),
        group: group.into(),
        endpoints: vec!["response".into(), "stability".into(), "mechanism".into()],
        values: values.into(),
    })
    .collect()
}

fn families() -> Vec<OrderedEndpointFamily> {
    vec![
        OrderedEndpointFamily {
            family: "primary".into(),
            endpoints: vec!["response".into(), "stability".into()],
        },
        OrderedEndpointFamily {
            family: "secondary".into(),
            endpoints: vec!["mechanism".into()],
        },
    ]
}

fn spec() -> MaxTPermutationSpec {
    MaxTPermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 43,
        alpha: 0.2,
    }
}

struct Reference {
    families: Vec<FamilyReference>,
    opened_family_count: usize,
}

struct FamilyReference {
    opened: bool,
    all_rejected: bool,
    critical_value: f64,
    adjusted_p_values: Vec<f64>,
    rejected: Vec<bool>,
}

fn slow_reference(
    patients: &[PatientEndpointVector],
    families: &[OrderedEndpointFamily],
    spec: &MaxTPermutationSpec,
    correction: MaxTCorrection,
) -> Reference {
    let observed_labels = patients
        .iter()
        .map(|patient| patient.group == spec.group_a)
        .collect::<Vec<_>>();
    let mut offset = 0usize;
    let mut opened = true;
    let mut family_results = Vec::new();
    for family in families {
        let endpoint_indices = offset..(offset + family.endpoints.len());
        let observed = endpoint_indices
            .clone()
            .map(|endpoint| statistic(patients, &observed_labels, endpoint).abs())
            .collect::<Vec<_>>();
        let order = observed_order(&observed);
        let mut exceedances = vec![0usize; observed.len()];
        let mut maxima = Vec::with_capacity(spec.permutations);
        for replicate in 0..spec.permutations {
            let labels = permuted_labels(&observed_labels, spec.seed, replicate);
            let null = endpoint_indices
                .clone()
                .map(|endpoint| statistic(patients, &labels, endpoint).abs())
                .collect::<Vec<_>>();
            let maximum = null.iter().copied().fold(0.0_f64, f64::max);
            maxima.push(maximum);
            match correction {
                MaxTCorrection::SingleStep => {
                    for (index, observed) in observed.iter().enumerate() {
                        exceedances[index] += usize::from(maximum >= *observed);
                    }
                }
                MaxTCorrection::StepDown => {
                    for (rank, endpoint) in order.iter().copied().enumerate() {
                        let remaining_maximum = order[rank..]
                            .iter()
                            .map(|index| null[*index])
                            .fold(0.0_f64, f64::max);
                        exceedances[endpoint] +=
                            usize::from(remaining_maximum >= observed[endpoint]);
                    }
                }
            }
        }
        let mut adjusted = exceedances
            .into_iter()
            .map(|count| (count as f64 + 1.0) / (spec.permutations + 1) as f64)
            .collect::<Vec<_>>();
        if correction == MaxTCorrection::StepDown {
            let mut previous = 0.0_f64;
            for endpoint in &order {
                previous = previous.max(adjusted[*endpoint]);
                adjusted[*endpoint] = previous;
            }
        }
        maxima.sort_by(f64::total_cmp);
        let rank = ((1.0 - spec.alpha) * (spec.permutations + 1) as f64).ceil() as usize;
        let critical_value = maxima[rank.saturating_sub(1).min(maxima.len() - 1)];
        let rejected = adjusted
            .iter()
            .map(|p_value| opened && *p_value <= spec.alpha)
            .collect::<Vec<_>>();
        let all_rejected = opened && rejected.iter().all(|value| *value);
        family_results.push(FamilyReference {
            opened,
            all_rejected,
            critical_value,
            adjusted_p_values: adjusted,
            rejected,
        });
        opened = all_rejected;
        offset += family.endpoints.len();
    }
    let opened_family_count = family_results.iter().filter(|family| family.opened).count();
    Reference {
        families: family_results,
        opened_family_count,
    }
}

fn statistic(patients: &[PatientEndpointVector], labels: &[bool], endpoint: usize) -> f64 {
    let group = |target| {
        patients
            .iter()
            .zip(labels)
            .filter_map(|(patient, label)| (*label == target).then_some(patient.values[endpoint]))
            .collect::<Vec<_>>()
    };
    let a = group(true);
    let b = group(false);
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let mean_a = mean(&a);
    let mean_b = mean(&b);
    let variance = |values: &[f64], mean: f64| {
        values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64
    };
    (mean_a - mean_b)
        / (variance(&a, mean_a) / a.len() as f64 + variance(&b, mean_b) / b.len() as f64).sqrt()
}

fn observed_order(statistics: &[f64]) -> Vec<usize> {
    let mut order = (0..statistics.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        statistics[*right]
            .total_cmp(&statistics[*left])
            .then_with(|| left.cmp(right))
    });
    order
}

fn permuted_labels(labels: &[bool], seed: u64, replicate: usize) -> Vec<bool> {
    let mut indices = (0..labels.len()).collect::<Vec<_>>();
    let mut state = splitmix64(splitmix64(seed ^ NAMESPACE) ^ replicate as u64);
    for index in (1..indices.len()).rev() {
        state = splitmix64(state ^ index as u64);
        let other = (state % (index as u64 + 1)) as usize;
        indices.swap(index, other);
    }
    indices.into_iter().map(|index| labels[index]).collect()
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}
