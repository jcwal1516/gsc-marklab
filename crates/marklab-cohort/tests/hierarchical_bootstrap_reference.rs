use marklab_cohort::{hierarchical_bootstrap, HierarchicalBootstrapSpec, HierarchicalScalarRecord};

const NAMESPACE: u64 = 0x6869_6572_5f62_6f6f;

#[test]
fn patient_first_replicates_and_interval_match_a_slow_reference() {
    let records = [
        ("p-1", "s-1", 1.0),
        ("p-1", "s-2", 3.0),
        ("p-2", "s-3", 5.0),
        ("p-2", "s-4", 7.0),
    ]
    .into_iter()
    .map(|(patient, specimen, endpoint)| HierarchicalScalarRecord {
        patient_id: patient.into(),
        specimen_id: specimen.into(),
        endpoint,
    })
    .collect::<Vec<_>>();
    let spec = HierarchicalBootstrapSpec {
        replicates: 199,
        seed: 59,
        alpha: 0.05,
    };
    let result = hierarchical_bootstrap(&records, &spec).expect("bootstrap result");
    let (means, lower, upper) = slow_reference(&spec);
    assert_eq!(result.bootstrap_means.len(), means.len());
    for (actual, expected) in result.bootstrap_means.iter().zip(means) {
        assert!((actual - expected).abs() < 1e-14);
    }
    assert!((result.interval.lower - lower).abs() < 1e-14);
    assert!((result.interval.upper - upper).abs() < 1e-14);
}

fn slow_reference(spec: &HierarchicalBootstrapSpec) -> (Vec<f64>, f64, f64) {
    let patients = [vec![1.0, 3.0], vec![5.0, 7.0]];
    let mut means = Vec::with_capacity(spec.replicates);
    for replicate in 0..spec.replicates {
        let mut state = derive_seed(spec.seed, replicate);
        let mut draw_index = 0usize;
        let mut values = Vec::new();
        for _ in 0..patients.len() {
            state = splitmix64(state ^ draw_index as u64);
            draw_index += 1;
            let specimens = &patients[state as usize % patients.len()];
            for _ in 0..specimens.len() {
                state = splitmix64(state ^ draw_index as u64);
                draw_index += 1;
                values.push(specimens[state as usize % specimens.len()]);
            }
        }
        means.push(values.iter().sum::<f64>() / values.len() as f64);
    }
    let mut ordered = means.clone();
    ordered.sort_by(f64::total_cmp);
    let lower = nearest_rank(&ordered, spec.alpha / 2.0);
    let upper = nearest_rank(&ordered, 1.0 - spec.alpha / 2.0);
    (means, lower, upper)
}

fn nearest_rank(ordered: &[f64], probability: f64) -> f64 {
    let rank = (probability * ordered.len() as f64).ceil() as usize;
    ordered[rank.saturating_sub(1).min(ordered.len() - 1)]
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
