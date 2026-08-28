use marklab_cohort::{
    cluster_level_permutation_test, ClusterPatientEndpoint, ClusterPermutationSpec,
    InferenceAnalysisLevel, InferenceNullFamily, InferencePermutationUnit, PermutationAlternative,
};

const NAMESPACE: u64 = 0x636c_7573_7465_725f;

#[test]
fn whole_cluster_permutation_matches_a_slow_reference_and_rejects_broken_hierarchies() {
    let records = records();
    let spec = ClusterPermutationSpec {
        group_a: "A".into(),
        group_b: "B".into(),
        permutations: 199,
        seed: 71,
        alternative: PermutationAlternative::TwoSided,
    };
    let result = cluster_level_permutation_test(&records, &spec).expect("cluster result");
    let values = [2.0, 4.0, 7.0, 1.0, 2.0, 3.0];
    let labels = [true, true, true, false, false, false];
    assert_eq!(result.p_value, slow_p_value(&values, &labels, &spec));
    assert!((result.effect_group_a_minus_group_b - 7.0 / 3.0).abs() < 1e-14);
    assert_eq!(
        result.inference_design.analysis_level(),
        InferenceAnalysisLevel::Cluster
    );
    assert_eq!(
        result.inference_design.null_family(),
        InferenceNullFamily::ClusterLabelPermutation
    );
    assert_eq!(
        result.inference_design.permutation_unit(),
        InferencePermutationUnit::CompleteClusterEndpoint
    );

    let mut duplicate = records.clone();
    duplicate[6].patient_id = "a-1".into();
    assert!(cluster_level_permutation_test(&duplicate, &spec).is_err());

    let mut conflicting = records.clone();
    conflicting[6].cluster_id = "cluster-a1".into();
    assert!(cluster_level_permutation_test(&conflicting, &spec).is_err());

    let too_few_a = records
        .into_iter()
        .filter(|record| record.group == "B" || record.cluster_id == "cluster-a1")
        .collect::<Vec<_>>();
    assert!(cluster_level_permutation_test(&too_few_a, &spec).is_err());
}

fn records() -> Vec<ClusterPatientEndpoint> {
    [
        ("a-1", "cluster-a1", "A", 1.0),
        ("a-2", "cluster-a1", "A", 3.0),
        ("a-3", "cluster-a2", "A", 4.0),
        ("a-4", "cluster-a3", "A", 5.0),
        ("a-5", "cluster-a3", "A", 7.0),
        ("a-6", "cluster-a3", "A", 9.0),
        ("b-1", "cluster-b1", "B", 0.0),
        ("b-2", "cluster-b1", "B", 2.0),
        ("b-3", "cluster-b2", "B", 2.0),
        ("b-4", "cluster-b3", "B", 1.0),
        ("b-5", "cluster-b3", "B", 3.0),
        ("b-6", "cluster-b3", "B", 5.0),
    ]
    .into_iter()
    .map(
        |(patient_id, cluster_id, group, endpoint)| ClusterPatientEndpoint {
            patient_id: patient_id.into(),
            cluster_id: cluster_id.into(),
            group: group.into(),
            endpoint,
        },
    )
    .collect()
}

fn slow_p_value(values: &[f64], labels: &[bool], spec: &ClusterPermutationSpec) -> f64 {
    let observed = welch(values, labels);
    let mut lower = 0usize;
    let mut upper = 0usize;
    for replicate in 0..spec.permutations {
        let mut sources = (0..labels.len()).collect::<Vec<_>>();
        let mut state = splitmix64(splitmix64(spec.seed ^ NAMESPACE) ^ replicate as u64);
        for index in (1..sources.len()).rev() {
            state = splitmix64(state ^ index as u64);
            let other = (state % (index as u64 + 1)) as usize;
            sources.swap(index, other);
        }
        let permuted = sources
            .iter()
            .map(|source| labels[*source])
            .collect::<Vec<_>>();
        let statistic = welch(values, &permuted);
        lower += usize::from(statistic <= observed);
        upper += usize::from(statistic >= observed);
    }
    let denominator = (spec.permutations + 1) as f64;
    (2.0 * ((lower.min(upper) as f64 + 1.0) / denominator)).min(1.0)
}

fn welch(values: &[f64], labels: &[bool]) -> f64 {
    let group = |target| {
        values
            .iter()
            .zip(labels)
            .filter_map(|(value, label)| (*label == target).then_some(*value))
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

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}
